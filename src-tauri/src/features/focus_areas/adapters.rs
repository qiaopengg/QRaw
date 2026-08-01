use rawler::decoders::RawMetadata;
use std::collections::HashMap;

use super::types::{FocusKind, FocusRegion};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Sony AF 提取
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Sony MakerNote 中已知的 AF 相关 tag
const SONY_AFPOINTS_SELECTED: u16 = 0xb700;
const SONY_AFPOINT: u16 = 0xb040;
const SONY_FOCUS_POSITION: u16 = 0xb701;
// Sony1 IFD (非加密) AF 相关标签 — 参考 ExifTool Sony.pm
const SONY_FLEXIBLE_SPOT_POSITION: u16 = 0x201d;
const SONY_AF_POINTS_USED: u16 = 0x2020;
const SONY_AF_POINT_SELECTED: u16 = 0x201e;

/// 从 0xB700 (AFPointsSelected) 提取
fn extract_sony_af_b700(data: &[u8], image_w: f32, _image_h: f32) -> Option<Vec<FocusRegion>> {
    if data.len() < 4 {
        return None;
    }
    let mut regions = Vec::new();

    // 格式 A: 简单 4 字节 (x%, y%, w%, h%) 各 0-255
    let x0 = data[0] as f32 / 255.0;
    let y0 = data[1] as f32 / 255.0;
    let w0 = data[2].max(1) as f32 / 255.0;
    let h0 = data[3].max(1) as f32 / 255.0;
    if x0 < 0.98 && y0 < 0.98 && w0 > 0.005 && w0 < 0.9 && h0 > 0.005 && h0 < 0.9 {
        regions.push(FocusRegion {
            x: x0,
            y: y0,
            width: w0,
            height: h0,
            kind: FocusKind::Area,
            is_primary: true,
        });
        return Some(regions);
    }

    // 格式 B: count + N*8 bytes each: (left, top, right, bottom) u16
    let count = data[0] as usize;
    if count > 0 && count <= 500 && data.len() >= 2 + count * 8 {
        let points = &data[2..];
        for i in 0..count {
            let off = i * 8;
            if off + 8 > points.len() {
                break;
            }
            let left = u16::from_le_bytes([points[off], points[off + 1]]) as f32;
            let top = u16::from_le_bytes([points[off + 2], points[off + 3]]) as f32;
            let right = u16::from_le_bytes([points[off + 4], points[off + 5]]) as f32;
            let bottom = u16::from_le_bytes([points[off + 6], points[off + 7]]) as f32;

            if left == 0.0 && top == 0.0 && right == 0.0 && bottom == 0.0 {
                continue;
            }

            // B700 AF 坐标可能基于 AF 网格(640×428)或图像像素，优先尝试 AF 网格尺度
            for &scale in &[640.0, image_w, 1000.0] {
                let x = left / scale;
                let y = top / scale;
                let w = (right - left).abs() / scale;
                let h = (bottom - top).abs() / scale;
                if x < 0.98 && y < 0.98 && w > 0.003 && w < 0.9 && h > 0.003 && h < 0.9 {
                    regions.push(FocusRegion {
                        x,
                        y,
                        width: w.max(0.01),
                        height: h.max(0.01),
                        kind: FocusKind::Area,
                        is_primary: i == 0,
                    });
                    break; // 找到有效尺度，跳出尺度循环
                }
            }
        }
        if !regions.is_empty() {
            return Some(regions);
        }
    }

    None
}

/// 从 0xB040 (AFPoint) 提取
fn extract_sony_af_b040(data: &[u8], image_w: f32, _image_h: f32) -> Option<FocusRegion> {
    if data.len() < 4 {
        return None;
    }
    // 尝试 u16×2 坐标 + u16×2 尺寸
    let x = u16::from_le_bytes([data[0], data[1]]) as f32;
    let y = u16::from_le_bytes([data[2], data[3]]) as f32;
    for &scale in &[image_w, 640.0, 1000.0, 8000.0] {
        let nx = x / scale;
        let ny = y / scale;
        if nx > 0.01 && nx < 0.98 && ny > 0.01 && ny < 0.98 {
            let w = if data.len() >= 8 {
                (u16::from_le_bytes([data[4], data[5]]) as f32 / scale).max(0.01)
            } else {
                0.03
            };
            let h = if data.len() >= 8 {
                (u16::from_le_bytes([data[6], data[7]]) as f32 / scale).max(0.01)
            } else {
                0.03
            };
            return Some(FocusRegion {
                x: nx,
                y: ny,
                width: w,
                height: h,
                kind: FocusKind::Point,
                is_primary: true,
            });
        }
    }
    None
}

/// Sony AF 提取主入口
/// 优先级: SubjectArea(在外部) > 0x201d FlexibleSpotPosition > 0xB700 > 0xB040 > 0xB701
/// sensor_w/sensor_h: 原生传感器尺寸(来自rawler),用于宽高比修正
pub(super) fn extract_sony_af(
    ifd: &HashMap<u16, Vec<u8>>,
    image_w: f32,
    image_h: f32,
    sensor_w: u32,
    sensor_h: u32,
) -> Vec<FocusRegion> {
    let mut regions = Vec::new();

    // 宽高比修正: 检测非原生宽高比拍摄 (参考 Focus-Points Fix #228)
    // Sony 全画幅原生宽高比为 3:2 (≈1.5), 如果 image 宽高比不同, 说明在机内做了裁切
    let (af_grid_w, af_grid_h, y_correction) = if sensor_w > 0 && sensor_h > 0 {
        let sensor_ratio = sensor_w as f32 / sensor_h as f32;
        let image_ratio = image_w / image_h;
        let ratio_diff = (sensor_ratio - image_ratio).abs();
        if ratio_diff > 0.02 {
            let sensor_aspect_w = image_w;
            let uncropped_h = sensor_aspect_w / sensor_ratio;
            let y_offset = ((uncropped_h - image_h) / 2.0).max(0.0);
            log::warn!(
                "Sony 宽高比修正: sensor={}x{} ratio={:.3}, image={:.0}x{:.0} ratio={:.3}, y_offset={:.0}/{:.0}",
                sensor_w,
                sensor_h,
                sensor_ratio,
                image_w,
                image_h,
                image_ratio,
                y_offset,
                uncropped_h
            );
            (640.0, 480.0, Some((y_offset, uncropped_h)))
        } else {
            (640.0, 480.0, None)
        }
    } else {
        (640.0, 480.0, None)
    };

    // 竖拍检测: image_w < image_h → 显示空间是竖拍, AF网格基于传感器横拍
    let is_portrait = image_w < image_h;

    // 0. Sony1 IFD: FlexibleSpotPosition (0x201d) — AF 网格坐标中心 → 输出左上角
    // AF网格 640×428 物理传感器, Y归一化用 480(等效高度)
    if let Some(raw) = ifd.get(&SONY_FLEXIBLE_SPOT_POSITION) {
        if raw.len() >= 4 {
            let fx = u16::from_le_bytes([raw[0], raw[1]]) as f32;
            let fy = u16::from_le_bytes([raw[2], raw[3]]) as f32;
            let cx = (fx / af_grid_w).clamp(0.0, 1.0);
            let mut cy = (fy / af_grid_h).clamp(0.0, 1.0);

            if let Some((y_offset, uncropped_h)) = y_correction {
                let sensor_y_px = cy * uncropped_h;
                let cropped_y_px = (sensor_y_px - y_offset).max(0.0);
                cy = (cropped_y_px / image_h).clamp(0.0, 1.0);
            }

            let marker = 0.015;
            let lx = (cx - marker).max(0.0);
            let ly = (cy - marker).max(0.0);

            // 竖拍时旋转传感器横拍坐标到显示空间 (默认270°CW)
            let (nx, ny) = if is_portrait {
                (ly, 1.0 - lx - 0.03)
            } else {
                (lx, ly)
            };

            if cx > 0.001 && cx < 0.999 && cy > 0.001 && cy < 0.999 {
                log::info!(
                    "Sony 0x201d FlexibleSpot → AF: raw=({:.0},{:.0}), sensor=({:.4},{:.4}), display=({:.4},{:.4})",
                    fx,
                    fy,
                    cx,
                    cy,
                    nx + marker,
                    ny + marker
                );
                regions.push(FocusRegion {
                    x: nx,
                    y: ny,
                    width: 0.03,
                    height: 0.03,
                    kind: FocusKind::Point,
                    is_primary: true,
                });
                return regions;
            } else if y_correction.is_some() {
                log::warn!(
                    "Sony 0x201d 经宽高比修正后超出图像范围: raw=({:.0},{:.0}), norm=({:.4},{:.4})",
                    fx,
                    fy,
                    cx,
                    cy
                );
            }
        }
    }

    // 1. AFPointsSelected (0xB700) — 选中的对焦区域列表
    if let Some(raw) = ifd.get(&SONY_AFPOINTS_SELECTED) {
        log::info!("Sony 0xB700: {} bytes", raw.len());
        if let Some(r) = extract_sony_af_b700(raw, image_w, image_h) {
            log::info!("Sony 0xB700 → {} AF 区域", r.len());
            regions.extend(r);
            return regions;
        }
    }

    // 2. AFPoint (0xB040) — 单一主对焦点
    if let Some(raw) = ifd.get(&SONY_AFPOINT) {
        log::info!("Sony 0xB040: {} bytes", raw.len());
        if let Some(r) = extract_sony_af_b040(raw, image_w, image_h) {
            log::info!(
                "Sony 0xB040 → AF: ({:.3},{:.3},{:.3},{:.3})",
                r.x,
                r.y,
                r.width,
                r.height
            );
            regions.push(r);
            return regions;
        }
    }

    // 3. FocusPosition (0xB701) — 对焦位置(百分比坐标)
    if let Some(raw) = ifd.get(&SONY_FOCUS_POSITION) {
        if raw.len() >= 4 {
            let x = u16::from_le_bytes([raw[0], raw[1]]) as f32 / 1000.0;
            let y = u16::from_le_bytes([raw[2], raw[3]]) as f32 / 1000.0;
            if x > 0.01 && x < 0.98 && y > 0.01 && y < 0.98 {
                log::info!("Sony 0xB701 → AF: ({:.3},{:.3})", x, y);
                regions.push(FocusRegion {
                    x,
                    y,
                    width: 0.04,
                    height: 0.04,
                    kind: FocusKind::Point,
                    is_primary: true,
                });
                return regions;
            }
        }
    }

    // 4. AFPointsUsed (0x2020) — bitmap, 记录对焦点使用情况 (仅用于诊断)
    if let Some(raw) = ifd.get(&SONY_AF_POINTS_USED) {
        let count = raw.iter().map(|&b| b.count_ones()).sum::<u32>();
        log::info!(
            "Sony 0x2020 AFPointsUsed: {} bytes, {} bits set",
            raw.len(),
            count
        );
    }

    // 5. AFPointSelected (0x201e) — 选中的对焦点序号 (仅用于诊断)
    if let Some(raw) = ifd.get(&SONY_AF_POINT_SELECTED) {
        if !raw.is_empty() {
            log::info!("Sony 0x201e AFPointSelected: {}", raw[0]);
        }
    }

    // 6. 加密标签 0x9416/0x940c/0x9405 — 不做解析，ExifTool 已在主路径处理

    regions
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Canon AF 提取
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const CANON_AFINFO: u16 = 0x0012;
const CANON_AFINFO2: u16 = 0x0026;
const CANON_AFINFO3: u16 = 0x003c;

pub(super) fn extract_canon_af(
    ifd: &HashMap<u16, Vec<u8>>,
    image_w: f32,
    image_h: f32,
) -> Vec<FocusRegion> {
    for &tag in &[CANON_AFINFO, CANON_AFINFO2, CANON_AFINFO3] {
        if let Some(data) = ifd.get(&tag) {
            if data.len() < 8 {
                continue;
            }
            let num_af = u16::from_le_bytes([data[0], data[1]]) as usize;
            let _valid = u16::from_le_bytes([data[2], data[3]]);
            let af_w = u16::from_le_bytes([data[4], data[5]]) as f32;
            let af_h = u16::from_le_bytes([data[6], data[7]]) as f32;
            if af_w <= 0.0 || af_h <= 0.0 || num_af == 0 || num_af > 200 {
                continue;
            }

            let mut regions = Vec::new();
            let pts = &data[8..];
            for i in 0..num_af {
                let o = i * 8;
                if o + 8 > pts.len() {
                    break;
                }
                let aw = u16::from_le_bytes([pts[o], pts[o + 1]]) as f32;
                let ah = u16::from_le_bytes([pts[o + 2], pts[o + 3]]) as f32;
                let ax = u16::from_le_bytes([pts[o + 4], pts[o + 5]]) as f32;
                let ay = u16::from_le_bytes([pts[o + 6], pts[o + 7]]) as f32;

                let x = ax / image_w;
                let y = ay / image_h;
                let w = (aw / image_w).max(0.005);
                let h = (ah / image_h).max(0.005);

                if x < 1.0 && y < 1.0 && w > 0.0 && w < 0.9 && h > 0.0 && h < 0.9 {
                    regions.push(FocusRegion {
                        x,
                        y,
                        width: w,
                        height: h,
                        kind: FocusKind::Area,
                        is_primary: i == 0,
                    });
                }
            }
            if !regions.is_empty() {
                log::info!("Canon 0x{:04x} → {} 对焦区域", tag, regions.len());
                return regions;
            }
        }
    }
    Vec::new()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Nikon AF 提取
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const NIKON_AFINFO: u16 = 0x0017;
const NIKON_AFPOINT: u16 = 0x0002;

pub(super) fn extract_nikon_af(
    ifd: &HashMap<u16, Vec<u8>>,
    image_w: f32,
    image_h: f32,
) -> Vec<FocusRegion> {
    if let Some(data) = ifd.get(&NIKON_AFINFO) {
        if data.len() >= 4 {
            let num_af = data[1] as usize;
            if num_af > 0 && num_af < 200 && data.len() >= 4 + num_af * 4 {
                let mut regions = Vec::new();
                let pts = &data[4..];
                for i in 0..num_af {
                    let o = i * 4;
                    if o + 4 > pts.len() {
                        break;
                    }
                    let x = pts[o] as f32 / 255.0;
                    let y = pts[o + 1] as f32 / 255.0;
                    let w = pts[o + 2].max(1) as f32 / 255.0;
                    let h = pts[o + 3].max(1) as f32 / 255.0;
                    if x < 0.98 && y < 0.98 && w > 0.005 && w < 0.9 && h > 0.005 && h < 0.9 {
                        regions.push(FocusRegion {
                            x,
                            y,
                            width: w,
                            height: h,
                            kind: FocusKind::Area,
                            is_primary: i == 0,
                        });
                    }
                }
                if !regions.is_empty() {
                    return regions;
                }
            }
        }
    }

    if let Some(data) = ifd.get(&NIKON_AFPOINT) {
        if data.len() >= 4 {
            let x = u16::from_le_bytes([data[0], data[1]]) as f32 / image_w;
            let y = u16::from_le_bytes([data[2], data[3]]) as f32 / image_h;
            if x < 0.98 && y < 0.98 {
                return vec![FocusRegion {
                    x: x.max(0.0),
                    y: y.max(0.0),
                    width: 0.03,
                    height: 0.03,
                    kind: FocusKind::Point,
                    is_primary: true,
                }];
            }
        }
    }
    Vec::new()
}
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Adapter trait
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub(super) trait FocusAdapter {
    fn supports(metadata: &RawMetadata) -> bool;
    #[allow(dead_code)]
    fn extract(
        metadata: &RawMetadata,
        raw_width: u32,
        raw_height: u32,
    ) -> Result<Vec<FocusRegion>, String>;
}

pub(super) struct SonyAdapter;
impl FocusAdapter for SonyAdapter {
    fn supports(metadata: &RawMetadata) -> bool {
        metadata.make.to_lowercase().contains("sony")
    }
    fn extract(
        _metadata: &RawMetadata,
        _raw_width: u32,
        _raw_height: u32,
    ) -> Result<Vec<FocusRegion>, String> {
        Ok(Vec::new())
    }
}

pub(super) struct CanonAdapter;
impl FocusAdapter for CanonAdapter {
    fn supports(metadata: &RawMetadata) -> bool {
        metadata.make.to_lowercase().contains("canon")
    }
    fn extract(
        _metadata: &RawMetadata,
        _raw_width: u32,
        _raw_height: u32,
    ) -> Result<Vec<FocusRegion>, String> {
        Ok(Vec::new())
    }
}

pub(super) struct NikonAdapter;
impl FocusAdapter for NikonAdapter {
    fn supports(metadata: &RawMetadata) -> bool {
        metadata.make.to_lowercase().contains("nikon")
    }
    fn extract(
        _metadata: &RawMetadata,
        _raw_width: u32,
        _raw_height: u32,
    ) -> Result<Vec<FocusRegion>, String> {
        Ok(Vec::new())
    }
}
