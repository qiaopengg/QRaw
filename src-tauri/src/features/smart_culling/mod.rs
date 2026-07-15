mod analysis;
mod face_models;
mod models;
mod types;

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use image_hasher::{HashAlg, HasherConfig};
use rayon::prelude::*;
use tauri::{AppHandle, Emitter};

use crate::app_settings::load_settings;
use crate::exif_processing::load_sidecar;
use crate::file_management::parse_virtual_path;
use crate::image_loader;

pub use models::SmartCullingFaceModels;
pub use types::{
    CullGroup, ImageAnalysisResult, SmartCullingApplyItem, SmartCullingProgress,
    SmartCullingSettings, SmartCullingSuggestions,
};

use analysis::analyze_image_quality;

struct ImageAnalysisData {
    hash: image_hasher::ImageHash,
    result: ImageAnalysisResult,
}

/// Score used to rank candidates within a similarity group for choosing the
/// representative (best) image. Closed eyes apply a fixed penalty so an
/// otherwise-sharper shot with closed eyes doesn't outrank an open-eyes shot
/// of similar quality.
const CLOSED_EYES_PENALTY: f64 = 0.3;

fn representative_rank_score(result: &ImageAnalysisResult) -> f64 {
    let has_closed_eyes = result.faces.iter().any(|face| face.is_closed);
    if has_closed_eyes {
        result.quality_score - CLOSED_EYES_PENALTY
    } else {
        result.quality_score
    }
}

fn analyze_single_image(
    path: &str,
    hasher: &image_hasher::Hasher,
    settings: &crate::app_settings::AppSettings,
    detect_faces: bool,
    face_models: Option<&SmartCullingFaceModels>,
) -> Result<ImageAnalysisData, String> {
    if crate::file_management::is_cloud_placeholder(std::path::Path::new(path)) {
        return Err(format!("'{}' is stored in iCloud and not downloaded", path));
    }

    let file_bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let img = image_loader::load_base_image_from_bytes(&file_bytes, path, true, settings, None)
        .map_err(|e| e.to_string())?;

    let analyzed = analyze_image_quality(&img, detect_faces, face_models);
    let hash_thumbnail = img.thumbnail(720, 720);
    let hash = hasher.hash_image(&hash_thumbnail);

    Ok(ImageAnalysisData {
        hash,
        result: ImageAnalysisResult {
            path: path.to_string(),
            quality_score: analyzed.quality_score,
            sharpness_metric: analyzed.sharpness_metric,
            center_focus_metric: analyzed.center_focus_metric,
            exposure_metric: analyzed.exposure_metric,
            width: analyzed.width,
            height: analyzed.height,
            faces: analyzed.faces,
        },
    })
}

pub async fn smart_culling_analyze(
    paths: Vec<String>,
    settings: SmartCullingSettings,
    app_handle: AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<SmartCullingSuggestions, String> {
    if paths.is_empty() {
        return Ok(SmartCullingSuggestions::default());
    }

    let app_settings = load_settings(app_handle.clone()).unwrap_or_default();

    let face_models: Option<Arc<SmartCullingFaceModels>> = if settings.detect_faces {
        let mut guard = state.smart_culling_face_models.lock().unwrap();
        if guard.is_none() {
            let models = models::load_face_models(&app_handle).map_err(|e| e.to_string())?;
            *guard = Some(models);
        }
        guard.clone()
    } else {
        None
    };

    let total_count = paths.len();
    let completed_count = Arc::new(AtomicUsize::new(0));
    let _ = app_handle.emit("smart-culling-start", total_count);

    let hasher = HasherConfig::new()
        .hash_alg(HashAlg::DoubleGradient)
        .hash_size(16, 16)
        .to_hasher();

    let analysis_results: Vec<Result<ImageAnalysisData, (String, String)>> = paths
        .par_iter()
        .map(|path| {
            let completed = completed_count.fetch_add(1, Ordering::Relaxed) + 1;
            let _ = app_handle.emit(
                "smart-culling-progress",
                SmartCullingProgress {
                    current: completed,
                    total: total_count,
                    stage: "Analyzing images...".to_string(),
                },
            );

            analyze_single_image(
                path,
                &hasher,
                &app_settings,
                settings.detect_faces,
                face_models.as_deref(),
            )
            .map_err(|e| (path.to_string(), e))
        })
        .collect();

    let mut successful_analyses = Vec::new();
    let mut failed_paths = Vec::new();
    for res in analysis_results {
        match res {
            Ok(data) => successful_analyses.push(data),
            Err((path, error)) => {
                log::warn!("Smart culling: failed to analyze image {}: {}", path, error);
                failed_paths.push(path);
            }
        }
    }

    let _ = app_handle.emit(
        "smart-culling-progress",
        SmartCullingProgress {
            current: total_count,
            total: total_count,
            stage: "Grouping similar images...".to_string(),
        },
    );

    let mut suggestions = SmartCullingSuggestions {
        failed_paths,
        ..Default::default()
    };
    let mut processed_indices = vec![false; successful_analyses.len()];

    if settings.group_similar {
        for i in 0..successful_analyses.len() {
            if processed_indices[i] {
                continue;
            }

            let mut current_group_indices = vec![];
            let mut queue = VecDeque::new();

            processed_indices[i] = true;
            current_group_indices.push(i);
            queue.push_back(i);

            while let Some(current_idx) = queue.pop_front() {
                for j in (current_idx + 1)..successful_analyses.len() {
                    if processed_indices[j] {
                        continue;
                    }

                    let dist = successful_analyses[current_idx]
                        .hash
                        .dist(&successful_analyses[j].hash);
                    if dist <= settings.similarity_threshold {
                        processed_indices[j] = true;
                        current_group_indices.push(j);
                        queue.push_back(j);
                    }
                }
            }

            if current_group_indices.len() > 1 {
                // Rank by quality score, but penalize images with closed eyes so a
                // sharp-but-closed-eyes shot isn't picked as the group's best over
                // an open-eyes alternative with similar quality.
                current_group_indices.sort_by(|&a, &b| {
                    representative_rank_score(&successful_analyses[b].result)
                        .partial_cmp(&representative_rank_score(&successful_analyses[a].result))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let representative_idx = current_group_indices[0];
                let duplicate_indices = &current_group_indices[1..];

                suggestions.similar_groups.push(CullGroup {
                    representative: successful_analyses[representative_idx].result.clone(),
                    duplicates: duplicate_indices
                        .iter()
                        .map(|&idx| successful_analyses[idx].result.clone())
                        .collect(),
                });
            }
        }
    }

    if settings.filter_blurry {
        for i in 0..successful_analyses.len() {
            if !processed_indices[i] {
                let item = &successful_analyses[i];
                if item.result.sharpness_metric < settings.blur_threshold {
                    suggestions.blurry_images.push(item.result.clone());
                }
            }
        }
        suggestions.blurry_images.sort_by(|a, b| {
            a.sharpness_metric
                .partial_cmp(&b.sharpness_metric)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    if settings.detect_faces {
        for analysis in &successful_analyses {
            if analysis.result.faces.iter().any(|face| face.is_closed) {
                suggestions.problem_faces.push(analysis.result.clone());
            }
        }
    }

    let _ = app_handle.emit("smart-culling-complete", &suggestions);
    Ok(suggestions)
}

/// Writes the smart-culling analysis outcome (score + reason + status) into
/// the image's `.rrdata` sidecar under the shared `featureData.smartCulling`
/// key, reusing the same `ImageMetadata`/`load_sidecar` machinery as the
/// rest of the app (ratings, color labels) instead of a bespoke persistence
/// mechanism.
pub fn smart_culling_write_metadata(items: Vec<SmartCullingApplyItem>) -> Result<(), String> {
    items.par_iter().for_each(|item| {
        let (_, sidecar_path) = parse_virtual_path(&item.path);
        let mut metadata = load_sidecar(&sidecar_path);

        let mut feature_data = metadata.feature_data.take().unwrap_or_else(|| serde_json::json!({}));
        if !feature_data.is_object() {
            feature_data = serde_json::json!({});
        }

        let smart_data = serde_json::json!({
            "score": item.score,
            "reasonText": item.reason_text,
            "status": item.status,
        });

        if let Some(obj) = feature_data.as_object_mut() {
            obj.insert("smartCulling".to_string(), smart_data);
        }
        metadata.feature_data = Some(feature_data);

        if let Ok(json_string) = serde_json::to_string_pretty(&metadata) {
            let _ = std::fs::write(&sidecar_path, json_string);
        }
    });

    Ok(())
}
