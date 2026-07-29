use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use image::{DynamicImage, GenericImageView};
use image_hasher::{HashAlg, Hasher, HasherConfig, ImageHash};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use super::analysis::analyze_image_quality;
use super::api::{DetectedFaceDto, FailureItem, KeyPersonSelection, ReviewResult};
use super::infrastructure::{CatalogAsset, render_current_state};
use super::models::SmartCullingFaceModels;
use super::scoring::{AnalysisCandidate, organize_results};
use crate::AppState;

pub(crate) struct RunOutcome {
    pub results: Vec<ReviewResult>,
    pub assets: HashMap<String, CatalogAsset>,
    pub failures: Vec<FailureItem>,
    pub completed: usize,
    pub total: usize,
    pub partial: bool,
}

pub(crate) fn run_analysis(
    app_handle: &AppHandle,
    root_path: &std::path::Path,
    mode: &str,
    assets: Vec<CatalogAsset>,
    key_people: &[KeyPersonSelection],
    models: &Arc<SmartCullingFaceModels>,
    cancellation: &Arc<AtomicBool>,
    mut on_progress: impl FnMut(usize, usize, &str),
) -> RunOutcome {
    let total = assets.len();
    let state = app_handle.state::<AppState>();
    let hasher = analysis_hasher();
    let key_hashes = build_key_hashes(&state, app_handle, key_people, &hasher);
    let mut analyzed = Vec::with_capacity(total);
    let mut asset_map = HashMap::with_capacity(total);
    let mut failures = Vec::new();
    let mut completed = 0;

    for asset in assets {
        if cancellation.load(Ordering::Acquire) {
            break;
        }
        let path = asset.primary_path.to_string_lossy().to_string();
        let member_paths = asset
            .member_paths
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        match render_current_state(&path, &state, app_handle) {
            Ok(image) => match analyze_image_quality(&image, true, Some(models)) {
                Ok(quality) => {
                    let hash = hasher.hash_image(&image.thumbnail(720, 720));
                    let result_id = Uuid::new_v4().to_string();
                    let key_person_priority =
                        match_key_person(&image, &quality.faces, &key_hashes, &hasher);
                    analyzed.push(AnalysisCandidate {
                        result_id: result_id.clone(),
                        path: asset.primary_path.clone(),
                        member_paths: asset.member_paths.clone(),
                        hash,
                        capture_time_millis: asset.capture_time_millis,
                        capture_time_from_exif: asset.capture_time_from_exif,
                        sequence_number: asset.sequence_number,
                        quality_score: quality.quality_score,
                        sharpness_metric: quality.sharpness_metric,
                        center_focus_metric: quality.center_focus_metric,
                        exposure_metric: quality.exposure_metric,
                        width: quality.width,
                        height: quality.height,
                        faces: quality.faces,
                        key_person_priority,
                    });
                    asset_map.insert(result_id, asset);
                }
                Err(error) => failures.push(FailureItem {
                    path,
                    member_paths,
                    stage: "analysis".to_string(),
                    code: "analysis_failed".to_string(),
                    detail: error.to_string(),
                    retryable: false,
                }),
            },
            Err(error) => failures.push(FailureItem {
                path,
                member_paths,
                stage: "render".to_string(),
                code: "render_failed".to_string(),
                detail: error.to_string(),
                retryable: false,
            }),
        }
        completed += 1;
        on_progress(completed, total, "analyzing");
    }

    on_progress(completed, total, "organizing");
    let results = organize_results(root_path, mode, analyzed);
    RunOutcome {
        results,
        assets: asset_map,
        failures,
        completed,
        total,
        partial: completed < total,
    }
}

pub(crate) fn detect_people(
    app_handle: &AppHandle,
    path: &str,
    models: &Arc<SmartCullingFaceModels>,
) -> Result<Vec<DetectedFaceDto>, String> {
    let state = app_handle.state::<AppState>();
    let image =
        render_current_state(path, &state, app_handle).map_err(|error| error.to_string())?;
    let (width, height) = image.dimensions();
    let analyzed =
        analyze_image_quality(&image, true, Some(models)).map_err(|error| error.to_string())?;
    Ok(analyzed
        .faces
        .into_iter()
        .map(|face| DetectedFaceDto {
            bbox: [
                face.bbox[0] / width as f32,
                face.bbox[1] / height as f32,
                face.bbox[2] / width as f32,
                face.bbox[3] / height as f32,
            ],
            score: face.eye_open_prob.unwrap_or(0.5),
            thumbnail_data_url: None,
        })
        .collect())
}

fn analysis_hasher() -> Hasher {
    HasherConfig::new()
        .hash_alg(HashAlg::DoubleGradient)
        .hash_size(16, 16)
        .to_hasher()
}

fn build_key_hashes(
    state: &tauri::State<'_, AppState>,
    app_handle: &AppHandle,
    selections: &[KeyPersonSelection],
    hasher: &Hasher,
) -> Vec<(usize, ImageHash)> {
    let mut hashes = Vec::new();
    for selection in selections {
        let Ok(image) = render_current_state(&selection.sample_path, state, app_handle) else {
            continue;
        };
        let Some(crop) = crop_normalized(&image, selection.bbox) else {
            continue;
        };
        hashes.push((
            selection.priority,
            hasher.hash_image(&crop.thumbnail(256, 256)),
        ));
    }
    hashes.sort_by_key(|(priority, _)| *priority);
    hashes
}

fn match_key_person(
    image: &DynamicImage,
    faces: &[super::types::FaceResult],
    key_hashes: &[(usize, ImageHash)],
    hasher: &Hasher,
) -> Option<usize> {
    if key_hashes.is_empty() {
        return None;
    }
    for face in faces {
        let (width, height) = image.dimensions();
        let bbox = [
            face.bbox[0] / width as f32,
            face.bbox[1] / height as f32,
            face.bbox[2] / width as f32,
            face.bbox[3] / height as f32,
        ];
        let Some(crop) = crop_normalized(image, bbox) else {
            continue;
        };
        let hash = hasher.hash_image(&crop.thumbnail(256, 256));
        if let Some((priority, _)) = key_hashes
            .iter()
            .map(|(priority, key_hash)| (*priority, hash.dist(key_hash)))
            .filter(|(_, distance)| *distance <= 52)
            .min_by_key(|(priority, distance)| (*distance, *priority))
        {
            return Some(priority);
        }
    }
    None
}

fn crop_normalized(image: &DynamicImage, bbox: [f32; 4]) -> Option<DynamicImage> {
    let (width, height) = image.dimensions();
    let x = (bbox[0].clamp(0.0, 1.0) * width as f32).floor() as u32;
    let y = (bbox[1].clamp(0.0, 1.0) * height as f32).floor() as u32;
    let crop_width = (bbox[2].clamp(0.0, 1.0) * width as f32).ceil() as u32;
    let crop_height = (bbox[3].clamp(0.0, 1.0) * height as f32).ceil() as u32;
    let crop_width = crop_width.min(width.saturating_sub(x));
    let crop_height = crop_height.min(height.saturating_sub(y));
    if crop_width == 0 || crop_height == 0 {
        return None;
    }
    Some(image.crop_imm(x, y, crop_width, crop_height))
}
