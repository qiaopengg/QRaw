use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use image::GenericImageView;
use image_hasher::{HashAlg, Hasher, HasherConfig};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use super::analysis::{analyze_image_quality, apply_source_clarity_metrics};
use super::api::{DetectedFaceDto, FailureItem, KeyPersonSelection, ReviewResult};
use super::face_identity::{KeyPersonReference, aggregate_reference_embeddings, match_key_people};
use super::infrastructure::{CatalogAsset, render_current_state};
use super::models::SmartCullingFaceModels;
use super::scoring::{AnalysisCandidate, organize_results};
use super::types::{KeyPersonEvidence, MIN_RELIABLE_FACE_DETECTION_SCORE};
use crate::AppState;
use crate::formats::is_raw_file;

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
    let include_identity = !key_people.is_empty();
    let mut analyzed = Vec::with_capacity(total);
    let mut asset_map = HashMap::with_capacity(total);
    let mut failures = Vec::new();
    let mut completed = 0;

    for asset in assets {
        if cancellation.load(Ordering::Acquire) {
            break;
        }
        let path = asset.display_path.to_string_lossy().to_string();
        let member_paths = asset
            .member_paths
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        match render_current_state(&path, &state, app_handle) {
            Ok(image) => {
                if cancellation.load(Ordering::Acquire) {
                    break;
                }
                match analyze_image_quality(
                    &image,
                    true,
                    include_identity,
                    Some(models),
                    Some(cancellation),
                ) {
                    Ok(mut quality) => {
                        if cancellation.load(Ordering::Acquire) {
                            break;
                        }
                        if !is_raw_file(&path)
                            && let Ok(source) = image::open(&asset.display_path)
                        {
                            apply_source_clarity_metrics(&source, &mut quality);
                        }
                        #[cfg(all(debug_assertions, target_os = "macos"))]
                        let vision_quality = super::vision_quality_poc::observe_calibration_image(
                            &image,
                            &quality.faces,
                        );
                        if cancellation.load(Ordering::Acquire) {
                            break;
                        }
                        let hash = hasher.hash_image(&image.thumbnail(720, 720));
                        let result_id = Uuid::new_v4().to_string();
                        analyzed.push(AnalysisCandidate {
                            result_id: result_id.clone(),
                            path: asset.display_path.clone(),
                            member_paths: asset.member_paths.clone(),
                            hash,
                            capture_time_millis: asset.capture_time_millis,
                            capture_time_from_exif: asset.capture_time_from_exif,
                            sequence_number: asset.sequence_number,
                            sharpness_metric: quality.sharpness_metric,
                            center_focus_metric: quality.center_focus_metric,
                            exposure_metric: quality.exposure_metric,
                            width: quality.width,
                            height: quality.height,
                            faces: quality.faces,
                            #[cfg(all(debug_assertions, target_os = "macos"))]
                            vision_quality,
                            key_person_evidence: Vec::new(),
                        });
                        asset_map.insert(result_id, asset);
                    }
                    Err(_) if cancellation.load(Ordering::Acquire) => break,
                    Err(error) => failures.push(FailureItem {
                        path,
                        member_paths,
                        stage: "analysis".to_string(),
                        code: "analysis_failed".to_string(),
                        detail: error.to_string(),
                        retryable: false,
                    }),
                }
            }
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

    if include_identity && !cancellation.load(Ordering::Acquire) {
        let (references, unavailable_priorities) = build_key_references(
            &state,
            app_handle,
            key_people,
            &analyzed,
            models,
            cancellation,
            &mut failures,
        );
        for candidate in &mut analyzed {
            if cancellation.load(Ordering::Acquire) {
                break;
            }
            candidate.key_person_evidence = match_key_people(&references, &mut candidate.faces);
            candidate
                .key_person_evidence
                .extend(
                    unavailable_priorities
                        .iter()
                        .map(|priority| KeyPersonEvidence {
                            priority: *priority,
                            face_index: None,
                            similarity: None,
                            status: "unknown".to_string(),
                            auto_score_eligible: false,
                            performance_rank: None,
                        }),
                );
            candidate
                .key_person_evidence
                .sort_by_key(|evidence| evidence.priority);
        }
    }

    if !cancellation.load(Ordering::Acquire) {
        on_progress(completed, total, "organizing");
    }
    let results = organize_results(root_path, mode, analyzed);
    RunOutcome {
        results,
        assets: asset_map,
        failures,
        completed,
        total,
        partial: completed < total || cancellation.load(Ordering::Acquire),
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
    let analyzed = analyze_image_quality(&image, true, false, Some(models), None)
        .map_err(|error| error.to_string())?;
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
            score: face.detection_score,
            thumbnail_data_url: None,
            landmarks: None,
            left_eye: None,
            right_eye: None,
            expression_state: None,
            expression_score: None,
            expression_confidence: None,
            expression_reason: None,
            sharpness_metric: None,
            sharpness_confidence: None,
            exposure_metric: None,
            exposure_confidence: None,
        })
        .collect())
}

pub(super) fn analysis_hasher() -> Hasher {
    HasherConfig::new()
        .hash_alg(HashAlg::DoubleGradient)
        .hash_size(16, 16)
        .to_hasher()
}

fn build_key_references(
    state: &tauri::State<'_, AppState>,
    app_handle: &AppHandle,
    selections: &[KeyPersonSelection],
    analyzed: &[AnalysisCandidate],
    models: &SmartCullingFaceModels,
    cancellation: &AtomicBool,
    failures: &mut Vec<FailureItem>,
) -> (Vec<KeyPersonReference>, Vec<usize>) {
    let mut references = Vec::new();
    let mut unavailable = Vec::new();
    let mut rendered_cache = HashMap::new();
    // One identity may contribute several reference photos, so embeddings are
    // collected per identity and aggregated into a single template afterwards.
    let mut embeddings_by_identity: HashMap<usize, Vec<Vec<f32>>> = HashMap::new();

    for selection in selections {
        if cancellation.load(Ordering::Acquire) {
            break;
        }
        let cached_face = analyzed
            .iter()
            .find(|candidate| candidate.path.to_string_lossy() == selection.sample_path)
            .and_then(|candidate| {
                find_selected_face(
                    &candidate.faces,
                    candidate.width,
                    candidate.height,
                    selection.bbox,
                )
            });
        let embedding = if let Some(face) = cached_face {
            face.identity_embedding.clone()
        } else {
            if !rendered_cache.contains_key(&selection.sample_path) {
                let analyzed_reference =
                    render_current_state(&selection.sample_path, state, app_handle)
                        .map_err(|error| error.to_string())
                        .and_then(|image| {
                            analyze_image_quality(
                                &image,
                                true,
                                true,
                                Some(models),
                                Some(cancellation),
                            )
                            .map_err(|error| error.to_string())
                        });
                rendered_cache.insert(selection.sample_path.clone(), analyzed_reference);
            }
            rendered_cache
                .get(&selection.sample_path)
                .and_then(|result| result.as_ref().ok())
                .and_then(|reference| {
                    find_selected_face(
                        &reference.faces,
                        reference.width,
                        reference.height,
                        selection.bbox,
                    )
                })
                .and_then(|face| face.identity_embedding.clone())
        };

        if let Some(embedding) = embedding {
            embeddings_by_identity
                .entry(selection.priority)
                .or_default()
                .push(embedding);
        } else {
            // A single unusable reference is reported but does not disqualify the
            // identity; it stays usable as long as another reference succeeded.
            failures.push(FailureItem {
                path: selection.sample_path.clone(),
                member_paths: vec![selection.sample_path.clone()],
                stage: "identity".to_string(),
                code: "reference_face_not_reacquired".to_string(),
                detail:
                    "The selected reference face could not be reacquired with sufficient overlap"
                        .to_string(),
                retryable: false,
            });
        }
    }

    let mut identities = selections
        .iter()
        .map(|selection| selection.priority)
        .collect::<Vec<_>>();
    identities.sort_unstable();
    identities.dedup();
    for identity in identities {
        let template = embeddings_by_identity
            .get(&identity)
            .and_then(|embeddings| aggregate_reference_embeddings(embeddings));
        match template {
            Some(embedding) => references.push(KeyPersonReference {
                priority: identity,
                embedding,
            }),
            None => unavailable.push(identity),
        }
    }

    references.sort_by_key(|reference| reference.priority);
    unavailable.sort_unstable();
    (references, unavailable)
}

fn find_selected_face(
    faces: &[super::types::FaceResult],
    width: u32,
    height: u32,
    selected_bbox: [f32; 4],
) -> Option<&super::types::FaceResult> {
    faces
        .iter()
        .filter(|face| {
            face.detection_score >= MIN_RELIABLE_FACE_DETECTION_SCORE
                && face
                    .landmarks
                    .iter()
                    .all(|point| point.0.is_finite() && point.1.is_finite())
        })
        .map(|face| {
            let bbox = [
                face.bbox[0] / width.max(1) as f32,
                face.bbox[1] / height.max(1) as f32,
                face.bbox[2] / width.max(1) as f32,
                face.bbox[3] / height.max(1) as f32,
            ];
            (face, bbox_iou(bbox, selected_bbox))
        })
        .filter(|(_, iou)| *iou >= 0.50)
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(face, _)| face)
}

fn bbox_iou(left: [f32; 4], right: [f32; 4]) -> f32 {
    let x1 = left[0].max(right[0]);
    let y1 = left[1].max(right[1]);
    let x2 = (left[0] + left[2]).min(right[0] + right[2]);
    let y2 = (left[1] + left[3]).min(right[1] + right[3]);
    let intersection = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let union = left[2] * left[3] + right[2] * right[3] - intersection;
    if union <= 0.0 {
        0.0
    } else {
        intersection / union
    }
}
