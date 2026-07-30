use std::collections::BTreeSet;

use super::scoring::AnalysisCandidate;

pub(crate) fn rank_key_person_performance(
    items: &mut [AnalysisCandidate],
    group_indices: &[usize],
) {
    let priorities = group_indices
        .iter()
        .flat_map(|index| {
            items[*index]
                .key_person_evidence
                .iter()
                .map(|evidence| evidence.priority)
        })
        .collect::<BTreeSet<_>>();

    for priority in priorities {
        let mut ranked = group_indices
            .iter()
            .filter_map(|index| {
                let candidate = &items[*index];
                let evidence = candidate
                    .key_person_evidence
                    .iter()
                    .find(|evidence| evidence.priority == priority)?;
                let face = candidate.faces.get(evidence.face_index?)?;
                Some((*index, face_performance(face)))
            })
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| right.1.total_cmp(&left.1));
        for (rank, (candidate_index, _)) in ranked.into_iter().enumerate() {
            if let Some(evidence) = items[candidate_index]
                .key_person_evidence
                .iter_mut()
                .find(|evidence| evidence.priority == priority)
            {
                evidence.performance_rank = Some(rank + 1);
            }
        }
    }
}

pub(crate) fn candidate_reason(item: &AnalysisCandidate) -> Option<String> {
    item.key_person_evidence
        .iter()
        .filter(|evidence| evidence.face_index.is_some())
        .min_by_key(|evidence| evidence.priority)
        .map(|evidence| {
            if evidence.status == "ambiguous" {
                format!("key_person_{}_ambiguous", evidence.priority)
            } else {
                format!("key_person_{}_candidate_review", evidence.priority)
            }
        })
}

fn face_performance(face: &super::types::FaceResult) -> f64 {
    let eye_score = [&face.left_eye, &face.right_eye]
        .iter()
        .map(|eye| match eye.state.as_str() {
            "open" => 1.0,
            "closed" => 0.0,
            _ => 0.5,
        })
        .sum::<f64>()
        / 2.0;
    let sharpness = ((face.sharpness_metric + 1.0).log10() / 3.5).clamp(0.0, 1.0);
    (eye_score * 0.40 + sharpness * 0.35 + face.exposure_metric.clamp(0.0, 1.0) * 0.25)
        .clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::smart_culling::types::{EyeResult, FaceResult};

    fn eye(state: &str) -> EyeResult {
        EyeResult {
            open_probability: None,
            state: state.to_string(),
            confidence: 1.0,
            effective_pixels: 100,
            sharpness_metric: Some(100.0),
        }
    }

    #[test]
    fn known_open_eyes_outrank_closed_eyes_when_other_signals_match() {
        let open = FaceResult {
            bbox: [0.0; 4],
            landmarks: [(0.0, 0.0); 5],
            detection_score: 1.0,
            left_eye: eye("open"),
            right_eye: eye("open"),
            expression_state: "unknown".to_string(),
            expression_confidence: 0.0,
            expression_reason: "model_unavailable".to_string(),
            sharpness_metric: 100.0,
            sharpness_confidence: 1.0,
            exposure_metric: 0.8,
            exposure_confidence: 1.0,
            identity_embedding: None,
        };
        let mut closed = open.clone();
        closed.left_eye = eye("closed");
        assert!(face_performance(&open) > face_performance(&closed));
    }
}
