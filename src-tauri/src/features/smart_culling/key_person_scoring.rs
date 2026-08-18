use std::collections::BTreeSet;

use super::mode_scoring::{Signal, combine, eye_signal, normalize_focus};
use super::scoring::AnalysisCandidate;

#[derive(Clone, Copy, Debug)]
struct FacePerformance {
    score: f64,
    confidence: f32,
}

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
        ranked.sort_by(|left, right| {
            right
                .1
                .score
                .total_cmp(&left.1.score)
                .then_with(|| right.1.confidence.total_cmp(&left.1.confidence))
        });
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

fn face_performance(face: &super::types::FaceResult) -> FacePerformance {
    let eyes = eye_signal(face);
    let (score, confidence) = combine(&[
        (eyes, 0.40),
        (
            Signal::available(
                normalize_focus(face.sharpness_metric),
                face.sharpness_confidence as f64,
            ),
            0.35,
        ),
        (
            Signal::available(face.exposure_metric, face.exposure_confidence as f64),
            0.25,
        ),
    ]);
    FacePerformance { score, confidence }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::smart_culling::types::{EyeDisposition, EyeResult, FaceResult};

    fn eye(state: &str) -> EyeResult {
        EyeResult {
            open_probability: None,
            state: state.to_string(),
            confidence: if state == "unknown" { 0.0 } else { 1.0 },
            reason: format!("eye_{state}_test"),
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
            eye_disposition: EyeDisposition::Open,
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
        closed.eye_disposition = EyeDisposition::Unusable;
        assert!(face_performance(&open).score > face_performance(&closed).score);
    }

    #[test]
    fn unknown_eye_evidence_is_omitted_instead_of_becoming_a_middle_score() {
        let known_open = FaceResult {
            bbox: [0.0; 4],
            landmarks: [(0.0, 0.0); 5],
            detection_score: 1.0,
            left_eye: eye("open"),
            right_eye: eye("open"),
            eye_disposition: EyeDisposition::Open,
            expression_state: "unknown".to_string(),
            expression_confidence: 0.0,
            expression_reason: "model_unavailable".to_string(),
            sharpness_metric: 100.0,
            sharpness_confidence: 1.0,
            exposure_metric: 0.8,
            exposure_confidence: 1.0,
            identity_embedding: None,
        };
        let mut unknown = known_open.clone();
        unknown.left_eye = eye("unknown");
        unknown.right_eye = eye("unknown");
        unknown.eye_disposition = EyeDisposition::Unknown;
        let mut known_closed = known_open.clone();
        known_closed.left_eye = eye("closed");
        known_closed.right_eye = eye("closed");
        known_closed.eye_disposition = EyeDisposition::Unusable;

        let open_performance = face_performance(&known_open);
        let unknown_performance = face_performance(&unknown);
        let closed_performance = face_performance(&known_closed);

        assert!(open_performance.score > unknown_performance.score);
        assert!(unknown_performance.score > closed_performance.score);
        assert!(unknown_performance.confidence < open_performance.confidence);
    }
}
