use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

use image_hasher::ImageHash;

use super::api::{DetectedFaceDto, ReviewResult};
use super::types::FaceResult;

pub(crate) const POLICY_VERSION: &str = "qraw-smart-culling-policy-2.0";
pub(crate) const MODEL_VERSION: &str = "yunet-2023mar+ocec-l";
const SIMILARITY_THRESHOLD: u32 = 28;

pub(crate) struct AnalysisCandidate {
    pub result_id: String,
    pub path: PathBuf,
    pub member_paths: Vec<PathBuf>,
    pub hash: ImageHash,
    pub quality_score: f64,
    pub sharpness_metric: f64,
    pub center_focus_metric: f64,
    pub exposure_metric: f64,
    pub width: u32,
    pub height: u32,
    pub faces: Vec<FaceResult>,
    pub key_person_priority: Option<usize>,
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
        folder_items.sort_by(|left, right| left.path.cmp(&right.path));
        let groups = similarity_groups(&folder_items);
        for (group_index, indices) in groups.into_iter().enumerate() {
            let mut ranked = indices
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
            let recommended_count = recommended_count(group_size);
            let group_id = format!("{}-{:04}", stable_folder_id(&folder), group_index + 1);
            let story = story_label(group_size, group_index + 1);

            for (rank, (candidate_index, score)) in ranked.into_iter().enumerate() {
                let candidate = &folder_items[candidate_index];
                let confidence = confidence(score, candidate);
                let rating = rating_for(score, rank, recommended_count);
                let color_label = color_for(rank, recommended_count, confidence);
                let reason_codes = reasons_for(candidate, rank, recommended_count, confidence);
                let resolved_mode = resolve_mode(mode, candidate);
                results.push(ReviewResult {
                    result_id: candidate.result_id.clone(),
                    path: candidate.path.to_string_lossy().to_string(),
                    member_paths: candidate
                        .member_paths
                        .iter()
                        .map(|path| path.to_string_lossy().to_string())
                        .collect(),
                    folder: folder.clone(),
                    story: story.clone(),
                    group_id: group_id.clone(),
                    group_size,
                    recommended_count,
                    rating,
                    color_label: Some(color_label.to_string()),
                    source: "ai".to_string(),
                    mode: resolved_mode.to_string(),
                    reason_codes,
                    confidence,
                    adopted: true,
                    protected: false,
                    width: candidate.width,
                    height: candidate.height,
                    faces: candidate
                        .faces
                        .iter()
                        .map(|face| DetectedFaceDto {
                            bbox: face.bbox,
                            score: face.eye_open_prob.unwrap_or(0.8),
                            thumbnail_data_url: None,
                        })
                        .collect(),
                });
            }
        }
    }

    results.sort_by(|left, right| left.path.cmp(&right.path));
    results
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

fn similarity_groups(items: &[AnalysisCandidate]) -> Vec<Vec<usize>> {
    let mut visited = vec![false; items.len()];
    let mut groups = Vec::new();
    for start in 0..items.len() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut queue = VecDeque::from([start]);
        let mut group = Vec::from([start]);
        while let Some(current) = queue.pop_front() {
            for next in (current + 1)..items.len() {
                if visited[next]
                    || items[current].hash.dist(&items[next].hash) > SIMILARITY_THRESHOLD
                {
                    continue;
                }
                visited[next] = true;
                queue.push_back(next);
                group.push(next);
            }
        }
        groups.push(group);
    }
    groups
}

fn mode_score(mode: &str, item: &AnalysisCandidate) -> f64 {
    let sharpness = (normalize_focus(item.sharpness_metric) * 0.8
        + item.quality_score.clamp(0.0, 1.0) * 0.2)
        .clamp(0.0, 1.0);
    let center = normalize_focus(item.center_focus_metric);
    let exposure = item.exposure_metric.clamp(0.0, 1.0);
    let open_eye = if item.faces.iter().any(|face| face.is_closed) {
        0.25
    } else if item.faces.is_empty() {
        0.65
    } else {
        1.0
    };
    let people = if item.faces.is_empty() { 0.4 } else { 1.0 };
    let key_person = item
        .key_person_priority
        .map(|priority| (1.0 - priority.saturating_sub(1) as f64 * 0.12).max(0.4))
        .unwrap_or(0.5);

    let (sharp_w, center_w, exposure_w, people_w, key_w) = match mode {
        "portrait" | "group" => (0.24, 0.16, 0.16, 0.26, 0.18),
        "environment" | "documentary" => (0.25, 0.16, 0.20, 0.22, 0.17),
        "wildlife" => (0.36, 0.22, 0.15, 0.17, 0.10),
        "landscape" | "architecture" | "astro" => (0.36, 0.28, 0.30, 0.04, 0.02),
        "product" => (0.38, 0.30, 0.26, 0.04, 0.02),
        _ if item.faces.is_empty() => (0.38, 0.28, 0.28, 0.04, 0.02),
        _ => (0.27, 0.17, 0.18, 0.23, 0.15),
    };
    let face_component = if item.faces.is_empty() {
        people
    } else {
        (people + open_eye) / 2.0
    };
    (sharpness * sharp_w
        + center * center_w
        + exposure * exposure_w
        + face_component * people_w
        + key_person * key_w)
        .clamp(0.0, 1.0)
}

fn normalize_focus(value: f64) -> f64 {
    ((value + 1.0).log10() / 3.5).clamp(0.0, 1.0)
}

fn recommended_count(group_size: usize) -> usize {
    match group_size {
        0 => 0,
        1..=5 => group_size,
        _ => ((group_size as f32 * 0.25).ceil() as usize).clamp(3, 5),
    }
}

fn confidence(score: f64, item: &AnalysisCandidate) -> f32 {
    let face_penalty = if item.faces.iter().any(|face| face.eye_open_prob.is_none()) {
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

fn color_for(rank: usize, recommended_count: usize, confidence: f32) -> &'static str {
    if confidence < 0.7 {
        "yellow"
    } else if rank < recommended_count {
        "green"
    } else if confidence >= 0.84 {
        "red"
    } else {
        "yellow"
    }
}

fn reasons_for(
    item: &AnalysisCandidate,
    rank: usize,
    recommended_count: usize,
    confidence: f32,
) -> Vec<String> {
    let mut reasons = Vec::with_capacity(2);
    if let Some(priority) = item.key_person_priority {
        reasons.push(format!("key_person_{priority}"));
    } else if item.faces.iter().any(|face| face.is_closed) {
        reasons.push("closed_eyes".to_string());
    } else if normalize_focus(item.sharpness_metric) >= 0.7 {
        reasons.push("sharp_subject".to_string());
    } else {
        reasons.push("soft_focus".to_string());
    }

    if rank == 0 {
        reasons.push("group_best".to_string());
    } else if rank < recommended_count {
        reasons.push("group_keeper".to_string());
    } else if confidence >= 0.84 {
        reasons.push("stronger_similar_exists".to_string());
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

fn story_label(group_size: usize, index: usize) -> String {
    if group_size > 1 {
        format!("similar:{index}")
    } else {
        format!("single:{index}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similar_groups_recommend_three_to_five_when_the_burst_is_large() {
        assert_eq!(recommended_count(12), 3);
        assert_eq!(recommended_count(20), 5);
        assert_eq!(recommended_count(40), 5);
    }

    #[test]
    fn low_confidence_never_becomes_a_red_reject() {
        assert_eq!(color_for(9, 3, 0.69), "yellow");
        assert_eq!(color_for(9, 3, 0.9), "red");
    }

    #[test]
    fn story_labels_are_stable_locale_independent_codes() {
        assert_eq!(story_label(4, 2), "similar:2");
        assert_eq!(story_label(1, 7), "single:7");
    }
}
