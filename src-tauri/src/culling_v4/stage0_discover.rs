use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use rayon::prelude::*;
use tauri::{AppHandle, Emitter};

use crate::exif_processing::{get_creation_date_from_path, read_exposure_time_secs, read_iso};
use crate::file_management::load_settings;
use crate::formats::is_raw_file;
use crate::image_loader::load_base_image_from_bytes;

use super::types::{Asset, AssetRegistry, CullingProgressV4};

const ANALYSIS_DIM: u32 = 720;

pub fn stage_0_discover(paths: &[String], app_handle: &AppHandle) -> Result<AssetRegistry, String> {
    if paths.is_empty() {
        return Ok(AssetRegistry {
            assets: vec![],
            raw_jpeg_pairs: HashMap::new(),
        });
    }

    let _ = app_handle.emit(
        "culling-progress",
        CullingProgressV4 {
            current: 0,
            total: paths.len(),
            stage: "Discovering assets...".into(),
        },
    );

    // ── Step 1: RAW+JPEG pairing ──
    let mut by_stem: HashMap<String, Vec<(String, bool)>> = HashMap::new();
    for path in paths {
        let p = Path::new(path);
        let stem = p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let is_raw = is_raw_file(p);
        by_stem
            .entry(stem)
            .or_default()
            .push((path.clone(), is_raw));
    }

    let mut primary_paths: Vec<String> = Vec::new();
    let mut raw_jpeg_pairs: HashMap<String, String> = HashMap::new();

    for (_, mut entries) in by_stem {
        // Sort: RAW files first
        entries.sort_by_key(|(_, is_raw)| if *is_raw { 0 } else { 1 });

        if entries.len() == 1 {
            primary_paths.push(entries[0].0.clone());
        } else {
            // Check time proximity for pairing
            let times: Vec<i64> = entries
                .iter()
                .map(|(p, _)| get_creation_date_from_path(Path::new(p)).timestamp_millis())
                .collect();

            let mut paired = vec![false; entries.len()];
            for i in 0..entries.len() {
                if paired[i] {
                    continue;
                }
                if entries[i].1 {
                    // This is a RAW file, look for JPEG pair
                    for j in 0..entries.len() {
                        if i == j || paired[j] || entries[j].1 {
                            continue;
                        }
                        if (times[i] - times[j]).abs() < 2000 {
                            // Pair found: RAW is primary, JPEG is secondary
                            raw_jpeg_pairs.insert(entries[i].0.clone(), entries[j].0.clone());
                            paired[j] = true;
                            break;
                        }
                    }
                    primary_paths.push(entries[i].0.clone());
                    paired[i] = true;
                } else if !paired[i] {
                    primary_paths.push(entries[i].0.clone());
                    paired[i] = true;
                }
            }
        }
    }

    // ── Step 2: Load thumbnails + EXIF in parallel ──
    let settings = load_settings(app_handle.clone()).unwrap_or_default();
    let hc = settings.raw_highlight_compression.unwrap_or(2.5);
    let lrm = settings.linear_raw_mode.clone();

    let total = primary_paths.len();
    let completed = std::sync::atomic::AtomicUsize::new(0);

    let assets: Vec<Option<Asset>> = primary_paths
        .par_iter()
        .map(|path| {
            let done = completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            let _ = app_handle.emit(
                "culling-progress",
                CullingProgressV4 {
                    current: done,
                    total,
                    stage: "Loading images...".into(),
                },
            );

            let p = Path::new(path);
            let stem = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let is_raw = is_raw_file(p);
            let is_primary = true;
            let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

            // EXIF
            let capture_time = get_creation_date_from_path(p).timestamp_millis();
            let file_bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(_) => return None,
            };
            let iso = read_iso(path, &file_bytes);
            let exposure_time = read_exposure_time_secs(path, &file_bytes);
            // focal_length not directly available from current exif_processing, set None
            let focal_length: Option<f32> = None;

            // Load and generate thumbnail
            let img =
                match load_base_image_from_bytes(&file_bytes, path, true, hc, lrm.clone(), None) {
                    Ok(i) => i,
                    Err(_) => return None,
                };

            let thumbnail = img.thumbnail(ANALYSIS_DIM, ANALYSIS_DIM);
            let gray_thumbnail = thumbnail.to_luma8();

            Some(Asset {
                path: path.clone(),
                stem,
                is_raw,
                is_primary,
                file_size,
                capture_time,
                iso,
                exposure_time,
                focal_length,
                thumbnail: Arc::new(thumbnail),
                gray_thumbnail: Arc::new(gray_thumbnail),
            })
        })
        .collect();

    let assets: Vec<Asset> = assets.into_iter().flatten().collect();

    Ok(AssetRegistry {
        assets,
        raw_jpeg_pairs,
    })
}
