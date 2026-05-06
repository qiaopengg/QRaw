use exif::{Tag, Value};
use rawler::decoders::{RawDecodeParams, RawMetadata};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::time::SystemTime;

use crate::exif_processing;
use crate::file_management::{parse_virtual_path, read_file_mapped};

/// 尝试从 rawler 读取原生传感器尺寸（含遮罩边距，非有效像素）
fn read_native_sensor_size(file_bytes: &[u8]) -> Option<(u32, u32)> {
    let source = rawler::rawsource::RawSource::new_from_slice(file_bytes);
    let decoder = rawler::get_decoder(&source).ok()?;
    let raw = decoder
        .raw_image(&source, &RawDecodeParams::default(), false)
        .ok()?;
    let w = raw.width as u32;
    let h = raw.height as u32;
    Some((w, h))
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  数据结构
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FocusRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub kind: FocusKind,
    pub is_primary: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FocusKind {
    Point,
    Area,
    Face,
    Eye,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  TIFF / MakerNote 解析
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn parse_tiff_exif_fields(file_bytes: &[u8]) -> Option<Vec<exif::Field>> {
    exif::parse_exif(file_bytes)
        .ok()
        .map(|(fields, _le)| fields)
}

fn find_field_value<'a>(fields: &'a [exif::Field], tag: Tag) -> Option<&'a Value> {
    fields.iter().find(|f| f.tag == tag).map(|f| &f.value)
}

/// 从 TIFF EXIF 获取图像尺寸
fn get_exif_dimensions(file_bytes: &[u8]) -> (u32, u32) {
    let fields = match parse_tiff_exif_fields(file_bytes) {
        Some(f) => f,
        None => return (0, 0),
    };
    let get_u32 = |tag| {
        find_field_value(&fields, tag).and_then(|v| match v {
            Value::Long(vals) if !vals.is_empty() => Some(vals[0]),
            Value::Short(vals) if !vals.is_empty() => Some(vals[0] as u32),
            _ => None,
        })
    };
    let w = get_u32(Tag::PixelXDimension)
        .or_else(|| get_u32(Tag::ImageWidth))
        .unwrap_or(0);
    let h = get_u32(Tag::PixelYDimension)
        .or_else(|| get_u32(Tag::ImageLength))
        .unwrap_or(0);
    (w, h)
}

/// 提取 MakerNote 原始二进制
fn extract_makernote_tiff(file_bytes: &[u8]) -> Option<Vec<u8>> {
    let fields = parse_tiff_exif_fields(file_bytes)?;
    match find_field_value(&fields, Tag::MakerNote)? {
        Value::Undefined(data, _) => Some(data.clone()),
        _ => None,
    }
}

/// 解析 MakerNote 中的 TIFF 子 IFD，返回 tag→原始字节 映射
fn parse_makernote_ifd(data: &[u8]) -> HashMap<u16, Vec<u8>> {
    let mut result = HashMap::new();
    if data.len() < 8 {
        return result;
    }

    // ── 跳过制造商前缀 ──
    let mut off: usize = 0;
    let mut endian_override: Option<bool> = None;

    if data.len() >= 9 && &data[0..9] == b"SONY DSC " {
        off = 12;
    } else if data.len() >= 8 && &data[0..8] == b"FUJIFILM" {
        off = 12;
    } else if data.len() >= 5 && &data[0..5] == b"OLYMP" {
        off = if data.len() >= 7 && &data[0..7] == b"OLYMPUS" {
            12
        } else {
            8
        };
    } else if data.len() >= 6 && &data[0..6] == b"PENTAX" {
        off = 8;
        if off + 2 <= data.len() {
            endian_override = Some(&data[off..off + 2] == b"II");
            off += 2;
        }
    } else if data.len() >= 7 && &data[0..7] == b"Nikon\0\x02" {
        off = 10;
        if off + 2 <= data.len() {
            endian_override = Some(&data[off..off + 2] == b"II");
            off += 2;
        }
    }
    if off >= data.len() {
        return result;
    }
    let payload = &data[off..];

    let little_endian = if let Some(le) = endian_override {
        le
    } else if payload.len() >= 2 && &payload[0..2] == b"II" {
        true
    } else if payload.len() >= 2 && &payload[0..2] == b"MM" {
        false
    } else {
        true // Sony 默认 LE
    };

    // 跳过 II/MM 标记
    let tiff_start = if endian_override.is_none()
        && payload.len() >= 2
        && (&payload[0..2] == b"II" || &payload[0..2] == b"MM")
    {
        2
    } else {
        0
    };
    let tiff_data = &payload[tiff_start..];
    if tiff_data.len() < 12 {
        return result;
    }

    // 确定 IFD 起始偏移
    let ifd_start = {
        let maybe_magic = if little_endian {
            u16::from_le_bytes([tiff_data[2], tiff_data[3]])
        } else {
            u16::from_be_bytes([tiff_data[2], tiff_data[3]])
        };
        if maybe_magic == 0x002a && tiff_data.len() >= 8 {
            if little_endian {
                u32::from_le_bytes([tiff_data[4], tiff_data[5], tiff_data[6], tiff_data[7]])
                    as usize
            } else {
                u32::from_be_bytes([tiff_data[4], tiff_data[5], tiff_data[6], tiff_data[7]])
                    as usize
            }
        } else {
            0
        }
    };
    if ifd_start + 2 > tiff_data.len() {
        return result;
    }

    // 读取 IFD 条目
    let entry_count = if little_endian {
        u16::from_le_bytes([tiff_data[ifd_start], tiff_data[ifd_start + 1]])
    } else {
        u16::from_be_bytes([tiff_data[ifd_start], tiff_data[ifd_start + 1]])
    } as usize;

    for i in 0..entry_count {
        let eo = ifd_start + 2 + i * 12;
        if eo + 12 > tiff_data.len() {
            break;
        }
        let tag = if little_endian {
            u16::from_le_bytes([tiff_data[eo], tiff_data[eo + 1]])
        } else {
            u16::from_be_bytes([tiff_data[eo], tiff_data[eo + 1]])
        };
        let vtype = if little_endian {
            u16::from_le_bytes([tiff_data[eo + 2], tiff_data[eo + 3]])
        } else {
            u16::from_be_bytes([tiff_data[eo + 2], tiff_data[eo + 3]])
        };
        let count = if little_endian {
            u32::from_le_bytes([
                tiff_data[eo + 4],
                tiff_data[eo + 5],
                tiff_data[eo + 6],
                tiff_data[eo + 7],
            ])
        } else {
            u32::from_be_bytes([
                tiff_data[eo + 4],
                tiff_data[eo + 5],
                tiff_data[eo + 6],
                tiff_data[eo + 7],
            ])
        } as usize;
        let val_bytes = &tiff_data[eo + 8..eo + 12];

        let type_size = match vtype {
            1 | 2 | 7 => 1,
            3 => 2,
            4 | 9 => 4,
            5 | 10 => 8,
            _ => 1,
        };
        let total = count * type_size;
        if total <= 4 {
            result.insert(tag, val_bytes[..total.min(4)].to_vec());
        } else {
            let val_off = if little_endian {
                u32::from_le_bytes([val_bytes[0], val_bytes[1], val_bytes[2], val_bytes[3]])
                    as usize
            } else {
                u32::from_be_bytes([val_bytes[0], val_bytes[1], val_bytes[2], val_bytes[3]])
                    as usize
            };
            if val_off + total <= tiff_data.len() {
                result.insert(tag, tiff_data[val_off..val_off + total].to_vec());
            }
        }
    }
    result
}

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
fn extract_sony_af(
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

fn extract_canon_af(ifd: &HashMap<u16, Vec<u8>>, image_w: f32, image_h: f32) -> Vec<FocusRegion> {
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

fn extract_nikon_af(ifd: &HashMap<u16, Vec<u8>>, image_w: f32, image_h: f32) -> Vec<FocusRegion> {
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
//  标准 EXIF 兜底: SubjectArea / SubjectLocation
//  这是最通用的对焦信息来源 — 几乎所有的相机都可能写入此 tag
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn extract_subject_area(file_bytes: &[u8], image_w: f32, image_h: f32) -> Vec<FocusRegion> {
    if image_w <= 0.0 || image_h <= 0.0 {
        return Vec::new();
    }

    let fields = match parse_tiff_exif_fields(file_bytes) {
        Some(f) => f,
        None => return Vec::new(),
    };

    if let Some(value) = find_field_value(&fields, Tag::SubjectArea) {
        if let Value::Short(vals) = value {
            if vals.len() >= 2 {
                // SubjectArea 坐标基于 EXIF 图像尺寸（与 normalize 维度一致）
                let x = vals[0] as f32 / image_w;
                let y = vals[1] as f32 / image_h;
                if x < 1.0 && y < 1.0 {
                    let w = if vals.len() >= 4 {
                        (vals[2] as f32 / image_w).max(0.02)
                    } else {
                        0.05
                    };
                    let h = if vals.len() >= 4 {
                        (vals[3] as f32 / image_h).max(0.02)
                    } else {
                        0.05
                    };
                    log::info!(
                        "SubjectArea → AF: ({:.4},{:.4},{:.4},{:.4}), image={:.0}x{:.0}",
                        x,
                        y,
                        w,
                        h,
                        image_w,
                        image_h
                    );
                    return vec![FocusRegion {
                        x,
                        y,
                        width: w,
                        height: h,
                        kind: FocusKind::Area,
                        is_primary: true,
                    }];
                }
            }
        }
    }

    if let Some(value) = find_field_value(&fields, Tag::SubjectLocation) {
        if let Value::Short(vals) = value {
            if vals.len() >= 2 {
                let x = vals[0] as f32 / image_w;
                let y = vals[1] as f32 / image_h;
                if x < 1.0 && y < 1.0 {
                    log::info!("SubjectLocation → AF: ({:.4},{:.4})", x, y);
                    return vec![FocusRegion {
                        x,
                        y,
                        width: 0.05,
                        height: 0.05,
                        kind: FocusKind::Point,
                        is_primary: true,
                    }];
                }
            }
        }
    }

    Vec::new()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Adapter trait
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub trait FocusAdapter {
    fn supports(metadata: &RawMetadata) -> bool;
    #[allow(dead_code)]
    fn extract(
        metadata: &RawMetadata,
        raw_width: u32,
        raw_height: u32,
    ) -> Result<Vec<FocusRegion>, String>;
}

pub struct SonyAdapter;
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

pub struct CanonAdapter;
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

pub struct NikonAdapter;
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Cache
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Debug)]
struct CacheEntry {
    regions: Vec<FocusRegion>,
    #[allow(dead_code)]
    modified: SystemTime,
}

pub struct FocusCache {
    cache: Mutex<HashMap<String, CacheEntry>>,
    order: Mutex<std::collections::VecDeque<String>>,
    max_size: usize,
}

impl FocusCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            order: Mutex::new(std::collections::VecDeque::new()),
            max_size,
        }
    }
    pub fn get(&self, key: &str) -> Option<Vec<FocusRegion>> {
        self.cache
            .lock()
            .unwrap()
            .get(key)
            .map(|e| e.regions.clone())
    }
    pub fn insert(&self, key: String, regions: Vec<FocusRegion>) {
        let mut cache = self.cache.lock().unwrap();
        let mut order = self.order.lock().unwrap();
        if let Some(pos) = order.iter().position(|k| k == &key) {
            order.remove(pos);
        }
        if cache.len() >= self.max_size {
            if let Some(oldest) = order.pop_front() {
                cache.remove(&oldest);
            }
        }
        order.push_back(key.clone());
        cache.insert(
            key,
            CacheEntry {
                regions,
                modified: SystemTime::now(),
            },
        );
    }
    #[allow(dead_code)]
    pub fn invalidate(&self, key: &str) {
        let mut cache = self.cache.lock().unwrap();
        let mut order = self.order.lock().unwrap();
        cache.remove(key);
        if let Some(pos) = order.iter().position(|k| k == key) {
            order.remove(pos);
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  ExifTool sidecar — 调用 exiftool 获取对焦坐标
//  所有坐标输出为左上角(L,T)，而非中心(Cx,Cy)
//  Y轴归一化使用 480(等效网格高度), 而非 428(物理传感器行数)
//  AF网格428行映射到图像的480等效单位, 覆盖约89%图像高度
//  优先级: FocusPixel(各品牌像素坐标) > FlexibleSpotPosition > FocalPlaneAFPoint > FocusLocation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn push_number_token(token: &mut String, values: &mut Vec<f32>) {
    if token.chars().any(|c| c.is_ascii_digit()) {
        if let Ok(value) = token.parse::<f32>() {
            values.push(value);
        }
    }
    token.clear();
}

fn numbers_from_string(input: &str) -> Vec<f32> {
    let mut values = Vec::new();
    let mut token = String::new();

    for ch in input.chars() {
        if ch.is_ascii_digit() || ch == '-' || ch == '+' || ch == '.' {
            token.push(ch);
        } else {
            push_number_token(&mut token, &mut values);
        }
    }
    push_number_token(&mut token, &mut values);
    values
}

fn numbers_from_json_value(value: &serde_json::Value) -> Vec<f32> {
    match value {
        serde_json::Value::Number(n) => n.as_f64().map(|v| vec![v as f32]).unwrap_or_default(),
        serde_json::Value::String(s) => numbers_from_string(s),
        serde_json::Value::Array(items) => items
            .iter()
            .flat_map(numbers_from_json_value)
            .collect::<Vec<_>>(),
        serde_json::Value::Object(map) => map
            .values()
            .flat_map(numbers_from_json_value)
            .collect::<Vec<_>>(),
        serde_json::Value::Bool(v) => vec![if *v { 1.0 } else { 0.0 }],
        serde_json::Value::Null => Vec::new(),
    }
}

fn string_from_json_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(v) => Some(if *v { "1".into() } else { "0".into() }),
        serde_json::Value::Array(items) => {
            let joined = items
                .iter()
                .filter_map(string_from_json_value)
                .collect::<Vec<_>>()
                .join(" ");
            (!joined.is_empty()).then_some(joined)
        }
        serde_json::Value::Object(map) => {
            let joined = map
                .values()
                .filter_map(string_from_json_value)
                .collect::<Vec<_>>()
                .join(" ");
            (!joined.is_empty()).then_some(joined)
        }
        serde_json::Value::Null => None,
    }
}

fn normalized_focus_region(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    kind: FocusKind,
    is_primary: bool,
) -> Option<FocusRegion> {
    if !x.is_finite() || !y.is_finite() || !width.is_finite() || !height.is_finite() {
        return None;
    }

    let mut nx = x;
    let mut ny = y;
    let mut nw = width.abs();
    let mut nh = height.abs();

    if nw <= 0.0 || nh <= 0.0 {
        return None;
    }
    if nx < 0.0 {
        nw += nx;
        nx = 0.0;
    }
    if ny < 0.0 {
        nh += ny;
        ny = 0.0;
    }
    if nx >= 1.0 || ny >= 1.0 {
        return None;
    }

    nw = nw.min(1.0 - nx);
    nh = nh.min(1.0 - ny);
    if nw < 0.001 || nh < 0.001 {
        return None;
    }

    Some(FocusRegion {
        x: nx,
        y: ny,
        width: nw,
        height: nh,
        kind,
        is_primary,
    })
}

fn focus_kind_from_mode(mode: Option<&str>) -> FocusKind {
    let mode = mode.unwrap_or_default().to_lowercase();
    if mode.contains("eye") {
        FocusKind::Eye
    } else if mode.contains("face") {
        FocusKind::Face
    } else if mode.contains("spot") || mode.contains("single") || mode.contains("flexible") {
        FocusKind::Point
    } else {
        FocusKind::Area
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exiftool_numeric_text() {
        assert_eq!(numbers_from_string("640x480"), vec![640.0, 480.0]);
        assert_eq!(
            numbers_from_string("-120 40, 320"),
            vec![-120.0, 40.0, 320.0]
        );
        assert_eq!(numbers_from_string("E9 (Center)"), vec![9.0]);
    }

    #[test]
    fn clamps_normalized_focus_region_to_visible_image() {
        let region = normalized_focus_region(-0.01, 0.25, 0.05, 0.1, FocusKind::Point, true)
            .expect("partially visible region should be kept");
        assert_eq!(region.x, 0.0);
        assert!((region.width - 0.04).abs() < f32::EPSILON);
    }

    #[test]
    fn infers_focus_kind_from_af_mode() {
        assert_eq!(
            focus_kind_from_mode(Some("Eye Detection AF")),
            FocusKind::Eye
        );
        assert_eq!(
            focus_kind_from_mode(Some("Face + Tracking")),
            FocusKind::Face
        );
        assert_eq!(
            focus_kind_from_mode(Some("Flexible Spot")),
            FocusKind::Point
        );
        assert_eq!(focus_kind_from_mode(Some("Zone AF")), FocusKind::Area);
    }
}

fn try_extract_via_exiftool(source_path: &Path) -> Result<Vec<FocusRegion>, String> {
    let output = Command::new("exiftool")
        .arg("-j")
        .arg("-Make")
        .arg("-Model")
        .arg("-ImageWidth")
        .arg("-ImageHeight")
        .arg("-ImageSize")
        .arg("-Orientation")
        .arg("-FocusPixel")
        .arg("-AFAreaXPosition")
        .arg("-AFAreaYPosition")
        .arg("-AFAreaXPositions")
        .arg("-AFAreaYPositions")
        .arg("-AFAreaWidth")
        .arg("-AFAreaHeight")
        .arg("-AFAreaWidths")
        .arg("-AFAreaHeights")
        .arg("-AFImageWidth")
        .arg("-AFImageHeight")
        .arg("-AFPointsInFocus")
        .arg("-AFPointsSelected")
        .arg("-PrimaryAFPoint")
        .arg("-AFDetectionMethod")
        .arg("-FocalPlaneAFPointArea")
        .arg("-FocalPlaneAFPointsUsed")
        .arg("-FocalPlaneAFPointLocation1")
        .arg("-FocalPlaneAFPointLocation2")
        .arg("-FocalPlaneAFPointLocation3")
        .arg("-FocalPlaneAFPointLocation4")
        .arg("-FocalPlaneAFPointLocation5")
        .arg("-FocalPlaneAFPointLocation6")
        .arg("-FocalPlaneAFPointLocation7")
        .arg("-FocalPlaneAFPointLocation8")
        .arg("-FocalPlaneAFPointLocation9")
        .arg("-FocalPlaneAFPointLocation10")
        .arg("-FocalPlaneAFPointLocation11")
        .arg("-FocalPlaneAFPointLocation12")
        .arg("-FocalPlaneAFPointLocation13")
        .arg("-FocalPlaneAFPointLocation14")
        .arg("-FocalPlaneAFPointLocation15")
        .arg("-FlexibleSpotPosition")
        .arg("-FocusLocation")
        .arg("-FocusFrameSize")
        .arg("-AFAreaMode")
        .arg(source_path)
        .output()
        .map_err(|e| format!("exiftool 进程启动失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("exiftool 退出码非零: {}", stderr.trim()));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("exiftool JSON 解析失败: {}", e))?;

    let entry = match json.as_array().and_then(|a| a.first()) {
        Some(e) => e,
        None => return Err("exiftool 返回空结果".into()),
    };

    let string_or =
        |key: &str| -> Option<String> { entry.get(key).and_then(string_from_json_value) };
    let numbers_or = |keys: &[&str]| -> Vec<f32> {
        keys.iter()
            .find_map(|key| {
                let nums = entry
                    .get(*key)
                    .map(numbers_from_json_value)
                    .unwrap_or_default();
                (!nums.is_empty()).then_some(nums)
            })
            .unwrap_or_default()
    };
    let first_number =
        |keys: &[&str]| -> Option<f32> { numbers_or(keys).into_iter().find(|v| v.is_finite()) };
    let dimensions_or =
        |width_keys: &[&str], height_keys: &[&str], size_keys: &[&str]| -> Option<(f32, f32)> {
            if let (Some(w), Some(h)) = (first_number(width_keys), first_number(height_keys))
                && w > 0.0
                && h > 0.0
            {
                return Some((w, h));
            }
            let size = numbers_or(size_keys);
            if size.len() >= 2 && size[0] > 0.0 && size[1] > 0.0 {
                Some((size[0], size[1]))
            } else {
                None
            }
        };

    let af_mode = string_or("AFAreaMode");
    let orientation = string_or("Orientation");
    let orientation_lc = orientation.clone().unwrap_or_default().to_lowercase();
    let orientation_num = first_number(&["Orientation"]).map(|v| v.round() as i32);
    let rot_270 = orientation_lc.contains("rotate 270") || orientation_num == Some(8);
    let rot_90 = orientation_lc.contains("rotate 90") || orientation_num == Some(6);
    let make_lc = string_or("Make").unwrap_or_default().to_lowercase();
    let model_lc = string_or("Model").unwrap_or_default().to_lowercase();
    let is_canon = make_lc.contains("canon");
    let is_nikon = make_lc.contains("nikon");
    let is_powershot = model_lc.contains("powershot");
    log::info!(
        "ExifTool: Make={:?}, Model={:?}, AFAreaMode={:?}, Orientation={:?}, path={}",
        string_or("Make"),
        string_or("Model"),
        af_mode,
        orientation,
        source_path.display()
    );

    // 传感器AF网格基于横拍, 竖拍时旋转坐标到显示空间
    // Rotate 270 CW: 传感器横轴上端→显示右端, 传感器纵轴不变方向→显示横轴
    //   (x,y,w,h) → (y, 1-x-w, h, w)
    // Rotate 90 CW: 传感器横轴下端→显示右端, 传感器纵轴翻转→显示横轴
    //   (x,y,w,h) → (1-y-h, x, h, w)
    let apply_orientation = |x: f32, y: f32, w: f32, h: f32| -> (f32, f32, f32, f32) {
        if rot_270 {
            (y, 1.0 - x - w, h, w)
        } else if rot_90 {
            (1.0 - y - h, x, h, w)
        } else {
            (x, y, w, h)
        }
    };

    // ── 0. FocusPixel (像素坐标, 品牌通用 — Fujifilm等) ──
    // FocusPixel 基准是 EXIF ImageWidth/ImageHeight (非 MakerNotes ImageSize)
    let focus_pixel = numbers_or(&["FocusPixel"]);
    if focus_pixel.len() >= 2 {
        if let Some((iw, ih)) = dimensions_or(&["ImageWidth"], &["ImageHeight"], &["ImageSize"]) {
            let cx = focus_pixel[0] / iw;
            let cy = focus_pixel[1] / ih;
            let marker = 0.02;
            let lx = cx - marker;
            let ly = cy - marker;
            let sz = 0.04;
            if cx >= 0.0 && cx <= 1.0 && cy >= 0.0 && cy <= 1.0 {
                let (nx, ny, nw, nh) = apply_orientation(lx, ly, sz, sz);
                if let Some(region) =
                    normalized_focus_region(nx, ny, nw, nh, FocusKind::Point, true)
                {
                    log::info!(
                        "ExifTool FocusPixel → AF: px=({:.0},{:.0})/{:.0}x{:.0}, display=({:.4},{:.4})",
                        focus_pixel[0],
                        focus_pixel[1],
                        iw,
                        ih,
                        region.x + region.width / 2.0,
                        region.y + region.height / 2.0
                    );
                    return Ok(vec![region]);
                }
            }
        }
    }

    // ── 0b. AFArea (Canon CR2/NEF, 中心原点坐标) ──
    // Canon AFAreaXPositions/YPositions 值为图像中心偏移量(负=左/上), 配合 AFImageWidth/Height
    // Nikon AFAreaXPosition/YPosition 为左上角原点像素坐标
    // AFPointsInFocus 为逗号分隔的索引(如 "35" 或 "0,1,2"), 指示哪些点合焦
    if let Some((iw, ih)) = dimensions_or(&["AFImageWidth"], &["AFImageHeight"], &[]) {
        let xpos = numbers_or(&["AFAreaXPositions"]);
        let ypos = numbers_or(&["AFAreaYPositions"]);
        let xpos_alt = numbers_or(&["AFAreaXPosition"]);
        let ypos_alt = numbers_or(&["AFAreaYPosition"]);
        let xp = if !xpos.is_empty() { xpos } else { xpos_alt };
        let yp = if !ypos.is_empty() { ypos } else { ypos_alt };
        let npts = xp.len().min(yp.len());

        if npts > 0 {
            let widths = numbers_or(&["AFAreaWidths", "AFAreaWidth"]);
            let heights = numbers_or(&["AFAreaHeights", "AFAreaHeight"]);
            let default_w = (iw * 0.04).max(1.0);
            let default_h = (ih * 0.04).max(1.0);
            let value_at = |values: &[f32], index: usize, default_value: f32| -> f32 {
                values
                    .get(index)
                    .copied()
                    .or_else(|| values.first().copied())
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .unwrap_or(default_value)
            };

            let focus_index_numbers = {
                let nums = numbers_or(&["AFPointsInFocus"]);
                if nums.is_empty() {
                    numbers_or(&["AFPointsSelected", "PrimaryAFPoint"])
                } else {
                    nums
                }
            };
            let raw_indices = focus_index_numbers
                .iter()
                .filter_map(|v| {
                    if v.is_finite() && *v >= 0.0 {
                        Some(v.round() as usize)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            let mut focus_indices = raw_indices
                .iter()
                .copied()
                .filter(|&i| i < npts)
                .collect::<Vec<_>>();
            if focus_indices.is_empty() && raw_indices.iter().all(|&i| i > 0 && i <= npts) {
                focus_indices = raw_indices.iter().map(|i| i - 1).collect();
            }
            if focus_indices.is_empty() && npts == 1 {
                focus_indices.push(0);
            }

            if !focus_indices.is_empty() {
                let af_detection = string_or("AFDetectionMethod")
                    .unwrap_or_default()
                    .to_lowercase();
                let nikon_center_position = is_nikon && af_detection.contains("contrast");
                let center_origin =
                    is_canon || xp.iter().any(|&v| v < 0.0) || yp.iter().any(|&v| v < 0.0);
                let kind = focus_kind_from_mode(af_mode.as_deref());
                let mut regions = Vec::new();

                for (rank, &fi) in focus_indices.iter().enumerate() {
                    if fi >= npts {
                        continue;
                    }
                    let pw = value_at(&widths, fi, default_w);
                    let ph = value_at(&heights, fi, default_h);
                    let half_w = pw / 2.0;
                    let half_h = ph / 2.0;
                    let x_off = xp[fi];
                    let y_off = yp[fi];

                    let (lx, ly) = if center_origin {
                        let y = if is_canon && !is_powershot {
                            ih / 2.0 - y_off - half_h
                        } else {
                            ih / 2.0 + y_off - half_h
                        };
                        (iw / 2.0 + x_off - half_w, y)
                    } else if nikon_center_position {
                        (x_off - half_w, y_off - half_h)
                    } else {
                        (x_off, y_off)
                    };

                    let (nx, ny, nw, nh) = apply_orientation(lx / iw, ly / ih, pw / iw, ph / ih);
                    if let Some(region) =
                        normalized_focus_region(nx, ny, nw, nh, kind.clone(), rank == 0)
                    {
                        regions.push(region);
                    }
                }

                if !regions.is_empty() {
                    log::info!(
                        "ExifTool AFArea → AF: {} of {} pts, mode={:?}",
                        regions.len(),
                        npts,
                        af_mode
                    );
                    return Ok(regions);
                }
            }
        }
    }

    // ── 1. FlexibleSpotPosition (640×428 网格, 用户对焦点中心 → 输出左上角) ──
    let flexible_spot = numbers_or(&["FlexibleSpotPosition"]);
    if flexible_spot.len() >= 2 && flexible_spot[0] > 0.0 && flexible_spot[1] > 0.0 {
        let cx = flexible_spot[0] / 640.0;
        let cy = flexible_spot[1] / 480.0;
        let marker = 0.015;
        let lx = cx - marker;
        let ly = cy - marker;
        if cx > 0.001 && cx < 0.999 && cy > 0.001 && cy < 0.999 {
            let (nx, ny, nw, nh) = apply_orientation(lx, ly, 0.03, 0.03);
            if let Some(region) = normalized_focus_region(nx, ny, nw, nh, FocusKind::Point, true) {
                log::info!(
                    "ExifTool FlexibleSpotPosition → AF: sensor=({:.4},{:.4}), display=({:.4},{:.4})",
                    cx,
                    cy,
                    region.x + region.width / 2.0,
                    region.y + region.height / 2.0
                );
                return Ok(vec![region]);
            }
        }
    }

    // ── 2. FocalPlaneAFPoint (640×428 网格, AF传感器区域 → 输出包围盒左上角) ──
    // grid_h 来自 ExifTool(FocalPlaneAFPointArea), 为物理428行
    // Y归一化必须用 norm_h=480(等效高度), 而非 grid_h=428
    let focal_plane_area = numbers_or(&["FocalPlaneAFPointArea"]);
    let grid_w: f32 = focal_plane_area.first().copied().unwrap_or(640.0);
    let norm_h: f32 = 480.0;

    let mut af_points: Vec<(f32, f32)> = Vec::new();
    for i in 1..=15 {
        let key = format!("FocalPlaneAFPointLocation{}", i);
        let parts = numbers_or(&[key.as_str()]);
        if parts.len() >= 2 && parts[0] > 0.0 && parts[1] > 0.0 {
            af_points.push((parts[0], parts[1]));
        }
    }

    if !af_points.is_empty() {
        let min_x = af_points.iter().map(|p| p.0).fold(f32::MAX, f32::min);
        let max_x = af_points.iter().map(|p| p.0).fold(0.0_f32, f32::max);
        let min_y = af_points.iter().map(|p| p.1).fold(f32::MAX, f32::min);
        let max_y = af_points.iter().map(|p| p.1).fold(0.0_f32, f32::max);

        let lx = (min_x / grid_w).max(0.0);
        let ly = (min_y / norm_h).max(0.0);
        let lw = ((max_x - min_x + 1.0) / grid_w).max(0.02);
        let lh = ((max_y - min_y + 1.0) / norm_h).max(0.02);
        let (nx, ny, nw, nh) = apply_orientation(lx, ly, lw, lh);

        log::info!(
            "ExifTool FocalPlaneAFPoint → AF: sensor_tl=({:.0},{:.0})_{:.0}x480, {} pts, display=({:.4},{:.4},{:.4},{:.4})",
            min_x,
            min_y,
            grid_w,
            af_points.len(),
            nx,
            ny,
            nw,
            nh
        );

        if let Some(region) = normalized_focus_region(nx, ny, nw, nh, FocusKind::Area, true) {
            return Ok(vec![region]);
        }
    }

    // ── 3. FocusLocation (像素坐标系, 焦点框中心 → 输出左上角) ──
    // FocusLocation 基于 IFD ImageWidth/Height(传感器横拍), 竖拍需旋转
    let focus_location = numbers_or(&["FocusLocation"]);
    if focus_location.len() >= 4 && focus_location[0] > 0.0 && focus_location[1] > 0.0 {
        let img_w = focus_location[0];
        let img_h = focus_location[1];
        let fx = focus_location[2];
        let fy = focus_location[3];

        let cenx = fx / img_w;
        let ceny = fy / img_h;

        let focus_frame_size = numbers_or(&["FocusFrameSize"]);
        let fw = focus_frame_size
            .first()
            .map(|v| (*v / img_w).max(0.01))
            .unwrap_or(0.05);
        let fh = focus_frame_size
            .get(1)
            .map(|v| (*v / img_h).max(0.01))
            .unwrap_or(0.05);

        let lx = cenx - fw / 2.0;
        let ly = ceny - fh / 2.0;
        let (nx, ny, nw, nh) = apply_orientation(lx, ly, fw, fh);

        if let Some(region) = normalized_focus_region(
            nx,
            ny,
            nw,
            nh,
            focus_kind_from_mode(af_mode.as_deref()),
            true,
        ) {
            log::info!(
                "ExifTool FocusLocation → AF: sensor=({:.4},{:.4},{:.4},{:.4}), display=({:.4},{:.4},{:.4},{:.4})",
                cenx,
                ceny,
                fw,
                fh,
                region.x,
                region.y,
                region.width,
                region.height
            );
            return Ok(vec![region]);
        }
    }

    Err("exiftool 未返回可解析的对焦坐标".into())
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Tauri 命令
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

static FOCUS_CACHE: once_cell::sync::Lazy<FocusCache> =
    once_cell::sync::Lazy::new(|| FocusCache::new(100));

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetFocusRegionsParams {
    path: String,
    #[serde(default)]
    image_width: Option<u32>,
    #[serde(default)]
    image_height: Option<u32>,
}

pub fn get_focus_regions(params: GetFocusRegionsParams) -> Result<Vec<FocusRegion>, String> {
    let (source_path, _sidecar) = parse_virtual_path(&params.path);
    let cache_key = format!("focus_{}", source_path.to_string_lossy());

    if let Some(cached) = FOCUS_CACHE.get(&cache_key) {
        log::debug!("对焦区域缓存命中: {:?}", source_path);
        return Ok(cached);
    }

    // ── 1. 优先: ExifTool sidecar (覆盖所有 AF 模式, 含加密标签解析) ──
    match try_extract_via_exiftool(&source_path) {
        Ok(regions) if !regions.is_empty() => {
            log::info!("ExifTool → {} 个对焦区域", regions.len());
            FOCUS_CACHE.insert(cache_key, regions.clone());
            return Ok(regions);
        }
        Ok(_) => {
            log::warn!("ExifTool 返回空结果，回退到内置解析");
        }
        Err(e) => {
            log::warn!("ExifTool 失败: {}，回退到内置解析", e);
        }
    }

    // ── 2. 内置回退: 标准 EXIF SubjectArea / SubjectLocation ──
    let file_bytes = match read_file_mapped(&source_path) {
        Ok(mmap) => mmap.to_vec(),
        Err(_) => std::fs::read(&source_path).map_err(|e| format!("无法读取文件: {}", e))?,
    };

    let raw_metadata =
        exif_processing::read_raw_metadata(&file_bytes).ok_or("不是 RAW 文件或元数据不可用")?;

    let (image_w, image_h) = if let (Some(w), Some(h)) = (params.image_width, params.image_height) {
        if w > 0 && h > 0 {
            (w as f32, h as f32)
        } else {
            let (ew, eh) = get_exif_dimensions(&file_bytes);
            (
                if ew > 0 { ew as f32 } else { 6000.0 },
                if eh > 0 { eh as f32 } else { 4000.0 },
            )
        }
    } else {
        let (ew, eh) = get_exif_dimensions(&file_bytes);
        (
            if ew > 0 { ew as f32 } else { 6000.0 },
            if eh > 0 { eh as f32 } else { 4000.0 },
        )
    };

    let regions = extract_subject_area(&file_bytes, image_w, image_h);
    if !regions.is_empty() {
        log::info!("内置 SubjectArea → {} 个对焦区域", regions.len());
        FOCUS_CACHE.insert(cache_key, regions.clone());
        return Ok(regions);
    }

    // ── 3. 内置回退: MakerNote 品牌特定 AF 标签 ──
    let mut regions = Vec::new();
    let maker_note = extract_makernote_tiff(&file_bytes);

    if let Some(ref mn) = maker_note {
        let ifd = parse_makernote_ifd(mn);
        if !ifd.is_empty() {
            let native_sensor = read_native_sensor_size(&file_bytes);
            let (sensor_w, sensor_h) = native_sensor.unwrap_or((0, 0));

            if SonyAdapter::supports(&raw_metadata) {
                regions = extract_sony_af(&ifd, image_w, image_h, sensor_w, sensor_h);
            } else if CanonAdapter::supports(&raw_metadata) {
                regions = extract_canon_af(&ifd, image_w, image_h);
            } else if NikonAdapter::supports(&raw_metadata) {
                regions = extract_nikon_af(&ifd, image_w, image_h);
            }
        }
    }

    if !regions.is_empty() {
        log::info!("内置 MakerNote → {} 个对焦区域", regions.len());
        FOCUS_CACHE.insert(cache_key, regions.clone());
        return Ok(regions);
    }

    // ── 4. 不支持的相机 → 静默返回空 ──
    log::info!("{} {} → 无对焦数据", raw_metadata.make, raw_metadata.model);
    Ok(Vec::new())
}
