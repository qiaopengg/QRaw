use serde::Deserialize;
use std::time::UNIX_EPOCH;

use super::adapters::{
    CanonAdapter, FocusAdapter, NikonAdapter, SonyAdapter, extract_canon_af, extract_nikon_af,
    extract_sony_af,
};
use super::cache::FocusCache;
use super::exiftool::try_extract_via_exiftool;
use super::metadata::{
    extract_makernote_tiff, get_exif_dimensions, parse_makernote_ifd, read_native_sensor_size,
};
use super::orientation::{get_exif_orientation, orient_focus_regions};
use super::standard_exif::extract_subject_area;
use super::types::FocusRegion;
use crate::exif_processing;
use crate::file_management::{parse_virtual_path, read_file_mapped};

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
    let file_modified = std::fs::metadata(&source_path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(UNIX_EPOCH);

    if let Some(cached) = FOCUS_CACHE.get(&cache_key, file_modified) {
        log::debug!("对焦区域缓存命中: {:?}", source_path);
        return Ok(cached);
    }

    // ── 1. 优先: ExifTool sidecar (覆盖所有 AF 模式, 含加密标签解析) ──
    match try_extract_via_exiftool(&source_path) {
        Ok(regions) if !regions.is_empty() => {
            log::info!("ExifTool → {} 个对焦区域", regions.len());
            FOCUS_CACHE.insert(cache_key, regions.clone(), file_modified);
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
    let mapped_bytes = read_file_mapped(&source_path).ok();
    let owned_bytes;
    let file_bytes: &[u8] = if let Some(ref mmap) = mapped_bytes {
        &mmap[..]
    } else {
        owned_bytes = std::fs::read(&source_path).map_err(|e| format!("无法读取文件: {}", e))?;
        &owned_bytes
    };

    let raw_metadata =
        exif_processing::read_raw_metadata(file_bytes).ok_or("不是 RAW 文件或元数据不可用")?;

    let orientation_code = get_exif_orientation(file_bytes);
    let (exif_w, exif_h) = get_exif_dimensions(file_bytes);
    let swaps_axes = matches!(orientation_code, 5 | 6 | 7 | 8);
    let (image_w, image_h) = if exif_w > 0 && exif_h > 0 {
        (exif_w as f32, exif_h as f32)
    } else if let (Some(w), Some(h)) = (params.image_width, params.image_height) {
        if w > 0 && h > 0 {
            if swaps_axes {
                (h as f32, w as f32)
            } else {
                (w as f32, h as f32)
            }
        } else {
            (6000.0, 4000.0)
        }
    } else {
        (6000.0, 4000.0)
    };

    let regions = orient_focus_regions(
        extract_subject_area(file_bytes, image_w, image_h),
        orientation_code,
    );
    if !regions.is_empty() {
        log::info!("内置 SubjectArea → {} 个对焦区域", regions.len());
        FOCUS_CACHE.insert(cache_key, regions.clone(), file_modified);
        return Ok(regions);
    }

    // ── 3. 内置回退: MakerNote 品牌特定 AF 标签 ──
    let mut regions = Vec::new();
    let maker_note = extract_makernote_tiff(file_bytes);

    if let Some(ref mn) = maker_note {
        let ifd = parse_makernote_ifd(mn);
        if !ifd.is_empty() {
            let native_sensor = read_native_sensor_size(file_bytes);
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

    let regions = orient_focus_regions(regions, orientation_code);
    if !regions.is_empty() {
        log::info!("内置 MakerNote → {} 个对焦区域", regions.len());
        FOCUS_CACHE.insert(cache_key, regions.clone(), file_modified);
        return Ok(regions);
    }

    // ── 4. 不支持的相机 → 静默返回空 ──
    log::info!("{} {} → 无对焦数据", raw_metadata.make, raw_metadata.model);
    FOCUS_CACHE.insert(cache_key, Vec::new(), file_modified);
    Ok(Vec::new())
}
