use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use image_hasher::ImageHash;

use super::api::{DetectedFaceDto, EyeEvidenceDto, KeyPersonEvidenceDto, ReviewResult};
use super::grouping::{CaptureDescriptor, group_capture_sequence};
use super::key_person_scoring::{
    candidate_reason, is_multi_person_comparable_group, rank_key_person_performance,
};
use super::types::{FaceResult, KeyPersonEvidence};

pub(crate) const POLICY_VERSION: &str = "qraw-smart-culling-policy-2.4";
pub(crate) const MODEL_VERSION: &str = "yunet-2023mar+ocec-l-bgr+sface-coreml-v3";
pub(crate) struct AnalysisCandidate {
    pub result_id: String,
    pub path: PathBuf,
    pub member_paths: Vec<PathBuf>,
    pub hash: ImageHash,
    pub capture_time_millis: i64,
    pub capture_time_from_exif: bool,
    pub sequence_number: Option<u64>,
    pub quality_score: f64,
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
            if is_multi_person_comparable_group(&folder_items, &group.indices) {
                rank_key_person_performance(&mut folder_items, &group.indices);
            } else {
                for index in &group.indices {
                    folder_items[*index].key_person_evidence.clear();
                }
            }
            let mut ranked = group
                .indices
                .iter()
                .map(|index| (*index, mode_score(mode, &folder_items[*index])))
                .collect::<Vec<_>>();
            ranked.sort_by(|left, right| {
                right
                    .1
                    .partial_cmp(&left.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let group_size = ranked.len();
            let group_kind = if group_size == 1 {
                "single"
            } else if capture_combination_suspected(&folder_items, &group.indices) {
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
                .map(|(rank, (candidate_index, score))| (*candidate_index, (rank, *score)))
                .collect::<BTreeMap<_, _>>();

            for candidate_index in group.indices {
                let candidate = &folder_items[candidate_index];
                let (rank, score) = rank_by_index[&candidate_index];
                let confidence = confidence(score, candidate);
                let rating = rating_for(score, rank, recommended_count);
                let color_label = color_for(rank, recommended_count, confidence, group_kind);
                let resolved_mode = resolve_mode(mode, candidate);
                let reason_codes = reasons_for(
                    candidate,
                    resolved_mode,
                    rank,
                    recommended_count,
                    group_size,
                    confidence,
                );
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
                    color_label: Some(color_label.to_string()),
                    source: "ai".to_string(),
                    mode: resolved_mode.to_string(),
                    reason_codes,
                    confidence,
                    ai_initially_adopted: group_kind == "reviewOnly" || rank < recommended_count,
                    adopted: group_kind == "reviewOnly" || rank < recommended_count,
                    protected: false,
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

fn resolve_mode<'a>(requested: &'a str, item: &AnalysisCandidate) -> &'a str {
    if requested != "auto" {
        return requested;
    }
    if item.faces.len() >= 3 {
        "group"
    } else if !item.faces.is_empty() {
        "portrait"
    } else if item.width > item.height {
        "landscape"
    } else {
        "product"
    }
}

fn mode_score(mode: &str, item: &AnalysisCandidate) -> f64 {
    let sharpness = (normalize_focus(item.sharpness_metric) * 0.8
        + item.quality_score.clamp(0.0, 1.0) * 0.2)
        .clamp(0.0, 1.0);
    let center = normalize_focus(item.center_focus_metric);
    let exposure = item.exposure_metric.clamp(0.0, 1.0);
    let open_eye = if item.faces.iter().any(FaceResult::has_closed_eye) {
        0.25
    } else if item.faces.is_empty() {
        0.65
    } else if item.faces.iter().any(|face| !face.eye_state_is_known()) {
        0.60
    } else {
        1.0
    };
    let people = if item.faces.is_empty() { 0.4 } else { 1.0 };
    // Identity candidates stay informational until the independent QRaw
    // photography validation gate passes, so identity presence has no weight.
    let (sharp_w, center_w, exposure_w, people_w) = match mode {
        "portrait" | "group" => (0.30, 0.20, 0.20, 0.30),
        "environment" | "documentary" => (0.30, 0.19, 0.24, 0.27),
        "wildlife" => (0.40, 0.24, 0.18, 0.18),
        "landscape" | "architecture" | "astro" => (0.38, 0.29, 0.31, 0.02),
        "product" => (0.39, 0.31, 0.28, 0.02),
        _ if item.faces.is_empty() => (0.39, 0.29, 0.30, 0.02),
        _ => (0.32, 0.20, 0.21, 0.27),
    };
    let face_component = if item.faces.is_empty() {
        people
    } else {
        (people + open_eye) / 2.0
    };
    (sharpness * sharp_w + center * center_w + exposure * exposure_w + face_component * people_w)
        .clamp(0.0, 1.0)
}

fn normalize_focus(value: f64) -> f64 {
    ((value + 1.0).log10() / 3.5).clamp(0.0, 1.0)
}

fn recommended_count(group_size: usize) -> usize {
    match group_size {
        0 => 0,
        1..=2 => group_size,
        3..=12 => 3,
        _ => ((group_size as f32 * 0.25).ceil() as usize).clamp(3, 5),
    }
}

fn confidence(score: f64, item: &AnalysisCandidate) -> f32 {
    let face_penalty = if item.faces.iter().any(|face| !face.eye_state_is_known()) {
        0.08
    } else {
        0.0
    };
    (0.58 + (score - 0.5).abs() * 0.72 - face_penalty).clamp(0.5, 0.96) as f32
}

fn rating_for(score: f64, rank: usize, recommended_count: usize) -> u8 {
    if rank == 0 && score >= 0.76 {
        5
    } else if rank < recommended_count && score >= 0.62 {
        4
    } else if rank < recommended_count || score >= 0.52 {
        3
    } else if score >= 0.38 {
        2
    } else {
        1
    }
}

fn color_for(
    rank: usize,
    recommended_count: usize,
    confidence: f32,
    group_kind: &str,
) -> &'static str {
    if group_kind == "reviewOnly" || confidence < 0.7 {
        "yellow"
    } else if rank < recommended_count {
        "green"
    } else {
        // Similar-group policy values have not passed the independent
        // real-photo release gate, so an unselected result is never a strong
        // red rejection.
        "yellow"
    }
}

fn reasons_for(
    item: &AnalysisCandidate,
    mode: &str,
    rank: usize,
    recommended_count: usize,
    group_size: usize,
    _confidence: f32,
) -> Vec<String> {
    let mut reasons = Vec::with_capacity(2);
    reasons.push(mode_reason(item, mode));

    if group_size <= 1 {
        reasons.extend(candidate_reason(item));
        return reasons;
    }

    if rank == 0 {
        reasons.push("group_best".to_string());
    } else if rank < recommended_count {
        reasons.push("group_keeper".to_string());
    } else if candidate_reason(item).is_some() {
        reasons.extend(candidate_reason(item));
    } else {
        reasons.push("needs_review".to_string());
    }
    reasons.truncate(2);
    reasons
}

fn mode_reason(item: &AnalysisCandidate, mode: &str) -> String {
    let sharp = normalize_focus(item.sharpness_metric) >= 0.7;
    let center_sharp = normalize_focus(item.center_focus_metric) >= 0.7;
    let exposure_balanced = item.exposure_metric >= 0.72;
    let has_closed_eyes = item.faces.iter().any(FaceResult::has_closed_eye);
    let eye_state_reliable =
        !item.faces.is_empty() && item.faces.iter().all(FaceResult::eye_state_is_known);

    let reason = match mode {
        "portrait" if has_closed_eyes => "portrait_closed_eyes",
        "portrait" if eye_state_reliable => "portrait_eyes_open",
        "portrait" if item.faces.is_empty() => "portrait_face_missing",
        "portrait" => "portrait_eye_review",
        "group" if has_closed_eyes => "group_closed_eyes",
        "group" if eye_state_reliable => "group_eyes_open",
        "group" => "group_eye_review",
        "environment" if has_closed_eyes => "environment_closed_eyes",
        "environment" if !item.faces.is_empty() && exposure_balanced => {
            "environment_people_exposure"
        }
        "environment" => "environment_balance_review",
        "documentary" if sharp => "documentary_moment_sharp",
        "documentary" => "documentary_moment_review",
        "landscape" if exposure_balanced => "landscape_exposure_balanced",
        "landscape" if sharp => "landscape_detail_strong",
        "landscape" => "landscape_detail_review",
        "wildlife" if sharp => "wildlife_detail_strong",
        "wildlife" => "wildlife_detail_review",
        "architecture" if center_sharp => "architecture_center_detail",
        "architecture" => "architecture_detail_review",
        "product" if center_sharp && exposure_balanced => "product_center_detail",
        "product" => "product_detail_review",
        "astro" => "astro_detail_review",
        _ if sharp => "sharp_subject",
        _ => "soft_focus",
    };
    reason.to_string()
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

fn capture_combination_suspected(items: &[AnalysisCandidate], indices: &[usize]) -> bool {
    if indices.len() < 2 {
        return false;
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
    max_exposure - min_exposure >= 0.25 || max_focus - min_focus >= 0.35
}

fn eye_dto(eye: &super::types::EyeResult) -> EyeEvidenceDto {
    EyeEvidenceDto {
        open_probability: eye.open_probability,
        state: eye.state.clone(),
        confidence: eye.confidence,
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
            bbox: [0.0; 4],
            landmarks: [(0.0, 0.0); 5],
            detection_score: 1.0,
            left_eye: super::super::types::EyeResult {
                open_probability: None,
                state: "open".to_string(),
                confidence: 1.0,
                effective_pixels: 100,
                sharpness_metric: Some(100.0),
            },
            right_eye: super::super::types::EyeResult {
                open_probability: None,
                state: "open".to_string(),
                confidence: 1.0,
                effective_pixels: 100,
                sharpness_metric: Some(100.0),
            },
            expression_state: "unknown".to_string(),
            expression_confidence: 0.0,
            expression_reason: "model_unavailable".to_string(),
            sharpness_metric: 100.0,
            sharpness_confidence: 1.0,
            exposure_metric: 0.8,
            exposure_confidence: 1.0,
            identity_embedding: None,
        }
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
            quality_score: 0.8,
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
            status: "matched".to_string(),
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
    fn unselected_results_remain_pending_until_grouping_release_gate_passes() {
        assert_eq!(color_for(9, 3, 0.69, "similar"), "yellow");
        assert_eq!(color_for(9, 3, 0.90, "similar"), "yellow");
        assert_eq!(color_for(0, 3, 0.90, "reviewOnly"), "yellow");
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

        assert_eq!(mode_reason(&item, "portrait"), "portrait_face_missing");
        assert_eq!(mode_reason(&item, "group"), "group_eye_review");
        assert_eq!(
            mode_reason(&item, "environment"),
            "environment_balance_review"
        );
        assert_eq!(
            mode_reason(&item, "landscape"),
            "landscape_exposure_balanced"
        );
        assert_eq!(
            mode_reason(&item, "documentary"),
            "documentary_moment_review"
        );
        assert_eq!(
            mode_reason(&item, "architecture"),
            "architecture_detail_review"
        );
        assert_eq!(mode_reason(&item, "wildlife"), "wildlife_detail_review");
        assert_eq!(mode_reason(&item, "product"), "product_detail_review");
        assert_eq!(mode_reason(&item, "astro"), "astro_detail_review");
    }

    #[test]
    fn single_photos_do_not_claim_a_group_comparison() {
        let item = candidate("frame.jpg", 1_000, 1);

        assert_eq!(
            reasons_for(&item, "landscape", 0, 1, 1, 0.9),
            vec!["landscape_exposure_balanced"]
        );
    }

    #[test]
    fn compared_photos_keep_mode_and_group_specific_reasons() {
        let item = candidate("frame.jpg", 1_000, 1);

        assert_eq!(
            reasons_for(&item, "documentary", 0, 3, 5, 0.9),
            vec!["documentary_moment_review", "group_best"]
        );
    }

    #[test]
    fn key_person_evidence_only_survives_real_multi_person_groups() {
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
                .all(|result| result.key_person_evidence.is_empty())
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
