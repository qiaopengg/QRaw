use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use image_hasher::ImageHash;

use super::api::{DetectedFaceDto, EyeEvidenceDto, KeyPersonEvidenceDto, ReviewResult};
use super::expression_grouping::apply_group_expression_sequences;
use super::grouping::{CaptureDescriptor, group_capture_sequence};
use super::key_person_policy::apply_key_person_gate;
use super::key_person_scoring::rank_key_person_performance;
use super::mode_scoring::{evaluate_mode, normalize_focus, rating_for_mode};
use super::types::{FaceResult, KeyPersonEvidence};

#[cfg(all(debug_assertions, target_os = "macos"))]
pub(crate) const POLICY_VERSION: &str = "qraw-smart-culling-policy-4.0-macos-calibration";
#[cfg(not(all(debug_assertions, target_os = "macos")))]
pub(crate) const POLICY_VERSION: &str = "qraw-smart-culling-policy-4.0-safe-evidence";
#[cfg(all(debug_assertions, target_os = "macos"))]
pub(crate) const MODEL_VERSION: &str = "yunet-2023mar+sface-2021dec+qraw-eye-model-contract-1.0+qraw-eye-policy-1.0+qraw-expression-descriptor-1.0+qraw-expression-sequence-policy-1.0";
#[cfg(not(all(debug_assertions, target_os = "macos")))]
pub(crate) const MODEL_VERSION: &str =
    "yunet-2023mar+ocec-loaded-unscored+sface-2021dec+eye-expression-unavailable-v1";
pub(crate) struct AnalysisCandidate {
    pub result_id: String,
    pub path: PathBuf,
    pub member_paths: Vec<PathBuf>,
    pub hash: ImageHash,
    pub capture_time_millis: i64,
    pub capture_time_from_exif: bool,
    pub sequence_number: Option<u64>,
    pub sharpness_metric: f64,
    pub center_focus_metric: f64,
    pub exposure_metric: f64,
    pub width: u32,
    pub height: u32,
    pub faces: Vec<FaceResult>,
    pub key_person_evidence: Vec<KeyPersonEvidence>,
}

pub(crate) fn organize_results(
    root: &Path,
    mode: &str,
    candidates: Vec<AnalysisCandidate>,
) -> Vec<ReviewResult> {
    let mut folders = BTreeMap::<String, Vec<AnalysisCandidate>>::new();
    for candidate in candidates {
        folders
            .entry(folder_label(root, &candidate.path))
            .or_default()
            .push(candidate);
    }

    let mut results = Vec::new();
    for (folder, mut folder_items) in folders {
        sort_capture_sequence(&mut folder_items);
        let descriptors = folder_items
            .iter()
            .map(|item| CaptureDescriptor {
                capture_time_millis: item.capture_time_millis,
                capture_time_from_exif: item.capture_time_from_exif,
                sequence_number: item.sequence_number,
                hash: &item.hash,
            })
            .collect::<Vec<_>>();
        for group in group_capture_sequence(&descriptors) {
            apply_group_expression_sequences(&mut folder_items, &group.indices, mode);
            if group.indices.len() > 1 {
                rank_key_person_performance(&mut folder_items, &group.indices);
            }
            let mut ranked = group
                .indices
                .iter()
                .map(|index| (*index, evaluate_mode(mode, &folder_items[*index])))
                .collect::<Vec<_>>();
            ranked.sort_by(|left, right| {
                right
                    .1
                    .score
                    .partial_cmp(&left.1.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let group_size = ranked.len();
            let capture_review_reason =
                capture_review_reason(&folder_items, &group.indices, group.requires_manual_review);
            let group_kind = if group_size == 1 {
                "single"
            } else if capture_review_reason.is_some() {
                "reviewOnly"
            } else {
                "similar"
            };
            let recommended_count = if group_kind == "reviewOnly" {
                group_size
            } else {
                recommended_count(group_size)
            };
            let group_id = format!("{}-{:04}", stable_folder_id(&folder), group.group_index);
            let rank_by_index = ranked
                .iter()
                .enumerate()
                .map(|(rank, (candidate_index, evaluation))| {
                    (*candidate_index, (rank, evaluation.clone()))
                })
                .collect::<BTreeMap<_, _>>();

            for candidate_index in group.indices {
                let candidate = &folder_items[candidate_index];
                let (rank, evaluation) = &rank_by_index[&candidate_index];
                let base_rating = rating_for_mode(&evaluation.resolved_mode, evaluation.score);
                let key_person = apply_key_person_gate(&candidate.key_person_evidence, base_rating);
                let requires_human_review = group_kind == "reviewOnly"
                    || evaluation.requires_human_review
                    || key_person.requires_human_review;
                let rating = if requires_human_review {
                    0
                } else {
                    key_person.rating
                };
                // AI color mapping is intentionally disabled until the product
                // policy is frozen. Colors never carry hidden pool or review semantics.
                let color_label = None;
                let reason_codes = if requires_human_review {
                    if group_kind == "reviewOnly" {
                        vec![
                            capture_review_reason
                                .unwrap_or("capture_combination_review")
                                .to_string(),
                        ]
                    } else {
                        reasons_for(
                            evaluation.reason_code,
                            key_person.reason_code.as_deref(),
                            *rank,
                            recommended_count,
                            group_size,
                        )
                    }
                } else {
                    reasons_for(
                        evaluation.reason_code,
                        key_person.reason_code.as_deref(),
                        *rank,
                        recommended_count,
                        group_size,
                    )
                };
                let confidence = if requires_human_review {
                    evaluation.confidence.min(0.49)
                } else {
                    evaluation.confidence
                };
                results.push(ReviewResult {
                    result_id: candidate.result_id.clone(),
                    path: candidate.path.to_string_lossy().to_string(),
                    member_paths: candidate
                        .member_paths
                        .iter()
                        .map(|path| path.to_string_lossy().to_string())
                        .collect(),
                    folder: folder.clone(),
                    group_id: group_id.clone(),
                    group_kind: group_kind.to_string(),
                    group_index: group.group_index,
                    group_rank: rank + 1,
                    group_size,
                    recommended_count,
                    rating,
                    color_label,
                    source: "ai".to_string(),
                    mode: evaluation.resolved_mode.clone(),
                    reason_codes,
                    confidence,
                    protected: false,
                    requires_human_review,
                    width: candidate.width,
                    height: candidate.height,
                    faces: candidate
                        .faces
                        .iter()
                        .map(|face| DetectedFaceDto {
                            bbox: normalize_bbox(face.bbox, candidate.width, candidate.height),
                            score: face.detection_score,
                            thumbnail_data_url: None,
                            landmarks: Some(normalize_landmarks(
                                face.landmarks,
                                candidate.width,
                                candidate.height,
                            )),
                            left_eye: Some(eye_dto(&face.left_eye)),
                            right_eye: Some(eye_dto(&face.right_eye)),
                            expression_state: Some(face.expression_state.clone()),
                            expression_confidence: Some(face.expression_confidence),
                            expression_reason: Some(face.expression_reason.clone()),
                            sharpness_metric: Some(face.sharpness_metric),
                            sharpness_confidence: Some(face.sharpness_confidence),
                            exposure_metric: Some(face.exposure_metric),
                            exposure_confidence: Some(face.exposure_confidence),
                        })
                        .collect(),
                    key_person_evidence: candidate
                        .key_person_evidence
                        .iter()
                        .map(|evidence| KeyPersonEvidenceDto {
                            priority: evidence.priority,
                            face_index: evidence.face_index,
                            similarity: evidence.similarity,
                            status: evidence.status.clone(),
                            auto_score_eligible: evidence.auto_score_eligible,
                            performance_rank: evidence.performance_rank,
                        })
                        .collect(),
                });
            }
        }
    }

    results
}

fn sort_capture_sequence(items: &mut [AnalysisCandidate]) {
    let use_filename_sequence = items.len() > 1
        && items
            .iter()
            .all(|item| !item.capture_time_from_exif && item.sequence_number.is_some());
    items.sort_by(|left, right| {
        if use_filename_sequence {
            left.sequence_number
                .cmp(&right.sequence_number)
                .then_with(|| left.path.cmp(&right.path))
        } else {
            left.capture_time_millis
                .cmp(&right.capture_time_millis)
                .then_with(|| left.path.cmp(&right.path))
        }
    });
}

fn recommended_count(group_size: usize) -> usize {
    match group_size {
        0 => 0,
        1..=2 => group_size,
        3..=12 => 3,
        _ => ((group_size as f32 * 0.25).ceil() as usize).clamp(3, 5),
    }
}

fn reasons_for(
    mode_reason: &str,
    key_person_reason: Option<&str>,
    rank: usize,
    recommended_count: usize,
    group_size: usize,
) -> Vec<String> {
    let mut reasons = Vec::with_capacity(2);
    if let Some(reason) = key_person_reason {
        reasons.push(reason.to_string());
    }
    if reasons.len() < 2 {
        reasons.push(mode_reason.to_string());
    }

    if group_size <= 1 {
        return reasons;
    }

    if reasons.len() >= 2 {
        return reasons;
    }
    if rank == 0 {
        reasons.push("group_best".to_string());
    } else if rank < recommended_count {
        reasons.push("group_keeper".to_string());
    } else {
        reasons.push("needs_review".to_string());
    }
    reasons.truncate(2);
    reasons
}

fn folder_label(root: &Path, path: &Path) -> String {
    path.parent()
        .and_then(|parent| parent.strip_prefix(root).ok())
        .map(|relative| {
            let value = relative.to_string_lossy();
            if value.is_empty() {
                ".".to_string()
            } else {
                value.to_string()
            }
        })
        .unwrap_or_else(|| ".".to_string())
}

fn stable_folder_id(folder: &str) -> String {
    blake3::hash(folder.as_bytes()).to_hex()[..8].to_string()
}

fn capture_review_reason(
    items: &[AnalysisCandidate],
    indices: &[usize],
    sequence_requires_review: bool,
) -> Option<&'static str> {
    if indices.len() < 2 {
        return None;
    }
    let (min_exposure, max_exposure) = indices
        .iter()
        .map(|index| items[*index].exposure_metric)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
            (min.min(value), max.max(value))
        });
    let normalized_focus = indices
        .iter()
        .map(|index| normalize_focus(items[*index].sharpness_metric))
        .collect::<Vec<_>>();
    let min_focus = normalized_focus
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let max_focus = normalized_focus
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    if max_exposure - min_exposure >= 0.25 {
        Some("exposure_bracket_review")
    } else if max_focus - min_focus >= 0.35 {
        Some("focus_stack_review")
    } else if sequence_requires_review {
        Some("panorama_or_action_review")
    } else {
        None
    }
}

fn eye_dto(eye: &super::types::EyeResult) -> EyeEvidenceDto {
    EyeEvidenceDto {
        open_probability: eye.open_probability,
        state: eye.state.clone(),
        confidence: eye.confidence,
        reason: eye.reason.clone(),
        effective_pixels: eye.effective_pixels,
        sharpness_metric: eye.sharpness_metric,
    }
}

fn normalize_landmarks(landmarks: [(f32, f32); 5], width: u32, height: u32) -> [[f32; 2]; 5] {
    if width == 0 || height == 0 {
        return [[0.0; 2]; 5];
    }
    landmarks.map(|(x, y)| {
        [
            (x / width as f32).clamp(0.0, 1.0),
            (y / height as f32).clamp(0.0, 1.0),
        ]
    })
}

fn normalize_bbox(bbox: [f32; 4], width: u32, height: u32) -> [f32; 4] {
    if width == 0 || height == 0 {
        return [0.0; 4];
    }
    [
        (bbox[0] / width as f32).clamp(0.0, 1.0),
        (bbox[1] / height as f32).clamp(0.0, 1.0),
        (bbox[2] / width as f32).clamp(0.0, 1.0),
        (bbox[3] / height as f32).clamp(0.0, 1.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face() -> FaceResult {
        FaceResult {
            bbox: [10.0, 10.0, 30.0, 30.0],
            landmarks: [(0.0, 0.0); 5],
            detection_score: 1.0,
            left_eye: super::super::types::EyeResult {
                open_probability: None,
                state: "open".to_string(),
                confidence: 1.0,
                reason: "eye_open_confident".to_string(),
                effective_pixels: 100,
                sharpness_metric: Some(100.0),
            },
            right_eye: super::super::types::EyeResult {
                open_probability: None,
                state: "open".to_string(),
                confidence: 1.0,
                reason: "eye_open_confident".to_string(),
                effective_pixels: 100,
                sharpness_metric: Some(100.0),
            },
            eye_disposition: super::super::types::EyeDisposition::Open,
            expression_state: "unknown".to_string(),
            expression_confidence: 0.0,
            expression_reason: "model_unavailable".to_string(),
            expression_descriptor: None,
            sharpness_metric: 100.0,
            sharpness_confidence: 1.0,
            exposure_metric: 0.8,
            exposure_confidence: 1.0,
            identity_embedding: None,
        }
    }

    #[test]
    fn result_versions_identify_the_seven_strategy_contract() {
        assert!(POLICY_VERSION.starts_with("qraw-smart-culling-policy-4.0-"));
        assert!(MODEL_VERSION.contains("yunet-2023mar"));
        assert!(MODEL_VERSION.contains("sface-2021dec"));
    }

    fn candidate(path: &str, capture_time_millis: i64, sequence_number: u64) -> AnalysisCandidate {
        AnalysisCandidate {
            result_id: path.to_string(),
            path: PathBuf::from(path),
            member_paths: Vec::new(),
            hash: ImageHash::from_bytes(&[0; 32]).unwrap(),
            capture_time_millis,
            capture_time_from_exif: false,
            sequence_number: Some(sequence_number),
            sharpness_metric: 1.0,
            center_focus_metric: 1.0,
            exposure_metric: 0.8,
            width: 100,
            height: 100,
            faces: Vec::new(),
            key_person_evidence: Vec::new(),
        }
    }

    fn key_person_candidate(
        path: &str,
        capture_time_millis: i64,
        sequence_number: u64,
        face_count: usize,
    ) -> AnalysisCandidate {
        let mut item = candidate(path, capture_time_millis, sequence_number);
        item.faces = (0..face_count).map(|_| face()).collect();
        item.key_person_evidence = vec![KeyPersonEvidence {
            priority: 1,
            face_index: Some(0),
            similarity: Some(0.9),
            status: "suspected".to_string(),
            auto_score_eligible: false,
            performance_rank: None,
        }];
        item
    }

    #[test]
    fn similar_groups_recommend_three_to_five_when_the_burst_is_large() {
        assert_eq!(recommended_count(2), 2);
        assert_eq!(recommended_count(3), 3);
        assert_eq!(recommended_count(5), 3);
        assert_eq!(recommended_count(12), 3);
        assert_eq!(recommended_count(20), 5);
        assert_eq!(recommended_count(40), 5);
    }

    #[test]
    fn ai_results_do_not_assign_unfrozen_color_semantics() {
        let results = organize_results(
            Path::new("."),
            "landscape",
            vec![candidate("frame.jpg", 1_000, 1)],
        );
        assert!(results.iter().all(|result| result.color_label.is_none()));
    }

    #[test]
    fn eye_unavailability_reason_survives_the_review_contract() {
        let eye =
            super::super::types::EyeResult::unavailable("eye_resolution_insufficient", 0, None);

        let dto = eye_dto(&eye);

        assert_eq!(dto.state, "unknown");
        assert_eq!(dto.confidence, 0.0);
        assert_eq!(dto.reason, "eye_resolution_insufficient");
    }

    #[test]
    fn capture_combinations_stay_zero_star_until_a_user_reviews_them() {
        let mut first = candidate("bracket-1.jpg", 1_000, 1);
        first.exposure_metric = 0.2;
        let mut second = candidate("bracket-2.jpg", 1_100, 2);
        second.exposure_metric = 0.8;

        let results = organize_results(Path::new("."), "landscape", vec![first, second]);

        assert_eq!(results.len(), 2);
        assert!(
            results
                .iter()
                .all(|result| result.group_kind == "reviewOnly")
        );
        assert!(results.iter().all(|result| result.rating == 0));
        assert!(results.iter().all(|result| result.color_label.is_none()));
        assert!(results.iter().all(|result| result.requires_human_review));
        assert!(
            results.iter().all(|result| {
                result.reason_codes == vec!["exposure_bracket_review".to_string()]
            })
        );
    }

    #[test]
    fn visually_different_adjacent_exif_frames_are_never_auto_rated() {
        let mut first = candidate("sequence-1.jpg", 1_000, 1);
        first.capture_time_from_exif = true;
        let mut second = candidate("sequence-2.jpg", 1_500, 2);
        second.capture_time_from_exif = true;
        let mut different_hash = [0_u8; 32];
        different_hash[..8].fill(255);
        second.hash = ImageHash::from_bytes(&different_hash).unwrap();

        let results = organize_results(Path::new("."), "landscape", vec![first, second]);

        assert_eq!(results.len(), 2);
        assert!(
            results
                .iter()
                .all(|result| result.group_kind == "reviewOnly")
        );
        assert!(results.iter().all(|result| result.rating == 0));
        assert!(results.iter().all(|result| {
            result.reason_codes == vec!["panorama_or_action_review".to_string()]
        }));
    }

    #[test]
    fn review_face_boxes_are_normalized_for_viewer_overlays() {
        assert_eq!(
            normalize_bbox([200.0, 100.0, 400.0, 300.0], 1_000, 500),
            [0.2, 0.2, 0.4, 0.6]
        );
    }

    #[test]
    fn fallback_timestamp_noise_does_not_scramble_numbered_images() {
        let mut items = vec![
            candidate("10.PNG", 600_000, 10),
            candidate("8.PNG", 1_000, 8),
            candidate("9.PNG", 300_000, 9),
        ];

        sort_capture_sequence(&mut items);

        assert_eq!(
            items
                .iter()
                .map(|item| item.sequence_number.unwrap())
                .collect::<Vec<_>>(),
            vec![8, 9, 10]
        );
    }

    #[test]
    fn shooting_modes_produce_mode_specific_primary_reasons() {
        let item = candidate("frame.jpg", 1_000, 1);

        assert_eq!(
            evaluate_mode("portrait", &item).reason_code,
            "portrait_subject_unreliable"
        );
        assert_eq!(
            evaluate_mode("group", &item).reason_code,
            "group_image_unclear"
        );
        assert_eq!(
            evaluate_mode("environment", &item).reason_code,
            "environment_subject_unreliable"
        );
        assert_eq!(
            evaluate_mode("landscape", &item).reason_code,
            "landscape_image_unclear"
        );
    }

    #[test]
    fn absolute_rating_does_not_depend_on_group_rank_or_selection_count() {
        assert_eq!(rating_for_mode("landscape", 0.80), 5);
        assert_eq!(rating_for_mode("landscape", 0.70), 4);
        assert_eq!(rating_for_mode("landscape", 0.57), 3);
        assert_eq!(rating_for_mode("landscape", 0.45), 2);
        assert_eq!(rating_for_mode("landscape", 0.20), 1);
    }

    #[test]
    fn single_photos_do_not_claim_a_group_comparison() {
        assert_eq!(
            reasons_for("landscape_exposure_balanced", None, 0, 1, 1),
            vec!["landscape_exposure_balanced"]
        );
    }

    #[test]
    fn compared_photos_keep_mode_and_group_specific_reasons() {
        assert_eq!(
            reasons_for("landscape_exposure_balanced", None, 0, 3, 5),
            vec!["landscape_exposure_balanced", "group_best"]
        );
    }

    #[test]
    fn key_person_evidence_survives_single_and_multi_person_groups() {
        let single_person_results = organize_results(
            Path::new("."),
            "group",
            vec![
                key_person_candidate("single-a.jpg", 1_000, 1, 1),
                key_person_candidate("single-b.jpg", 1_100, 2, 1),
            ],
        );
        assert!(
            single_person_results
                .iter()
                .all(|result| !result.key_person_evidence.is_empty())
        );

        let multi_person_results = organize_results(
            Path::new("."),
            "group",
            vec![
                key_person_candidate("multi-a.jpg", 1_000, 1, 2),
                key_person_candidate("multi-b.jpg", 1_100, 2, 1),
            ],
        );
        assert!(
            multi_person_results
                .iter()
                .all(|result| !result.key_person_evidence.is_empty())
        );
    }
}
