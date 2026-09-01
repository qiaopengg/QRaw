use std::collections::BTreeSet;

use super::mode_evidence::{expression_signal, eye_signal};
use super::quality_evidence::{
    ScoreEvidence, WeightedEvidence, combine_fixed_weights, composition_evidence_unavailable,
    legacy_optical_evidence,
};
use super::scoring::AnalysisCandidate;

const RANKING_SIGNAL_VERSION: &str = "key_person_ranking_v1";

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
                .filter(|evidence| evidence.status == "confirmed" && evidence.auto_score_eligible)
                .map(|evidence| evidence.priority)
        })
        .collect::<BTreeSet<_>>();

    for priority in priorities {
        let mut ranked = group_indices
            .iter()
            .filter_map(|index| {
                let candidate = &items[*index];
                let evidence = candidate.key_person_evidence.iter().find(|evidence| {
                    evidence.priority == priority
                        && evidence.status == "confirmed"
                        && evidence.auto_score_eligible
                })?;
                let face = candidate.faces.get(evidence.face_index?)?;
                let optical =
                    legacy_optical_evidence(candidate.sharpness_metric, candidate.exposure_metric);
                Some((*index, face_performance(face, &optical)))
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
            if let Some(evidence) =
                items[candidate_index]
                    .key_person_evidence
                    .iter_mut()
                    .find(|evidence| {
                        evidence.priority == priority
                            && evidence.status == "confirmed"
                            && evidence.auto_score_eligible
                    })
            {
                evidence.performance_rank = Some(rank + 1);
            }
        }
    }
}

fn face_performance(face: &super::types::FaceResult, optical: &ScoreEvidence) -> FacePerformance {
    let eyes = eye_signal(face)
        .map(|(score, confidence)| available(score, confidence))
        .unwrap_or_else(|| {
            ScoreEvidence::unavailable("key_person_eye_state_unknown", RANKING_SIGNAL_VERSION)
        });
    let expression = expression_signal(face)
        .map(|(score, confidence)| available(score, confidence))
        .unwrap_or_else(|| {
            ScoreEvidence::unavailable(
                "key_person_expression_state_unknown",
                RANKING_SIGNAL_VERSION,
            )
        });
    let composition = composition_evidence_unavailable();
    let interval = combine_fixed_weights(&[
        WeightedEvidence {
            evidence: &eyes,
            weight: 0.40,
        },
        WeightedEvidence {
            evidence: &expression,
            weight: 0.40,
        },
        WeightedEvidence {
            evidence: optical,
            weight: 0.10,
        },
        WeightedEvidence {
            evidence: &composition,
            weight: 0.10,
        },
    ])
    .expect("key-person ranking weights and evidence must validate");
    FacePerformance {
        // Ranking needs a total order, while star assignment retains the full
        // interval. The midpoint keeps unknown eyes between known-open and
        // known-closed frames without re-normalizing the remaining signals.
        score: (interval.lower_bound + interval.upper_bound) * 0.5,
        confidence: interval.confidence as f32,
    }
}

fn available(score: f64, confidence: f64) -> ScoreEvidence {
    ScoreEvidence::try_available(
        score.clamp(0.0, 1.0),
        confidence.clamp(0.0, 1.0),
        "key_person_ranking_signal",
        RANKING_SIGNAL_VERSION,
    )
    .unwrap_or_else(|_| {
        ScoreEvidence::unavailable("key_person_ranking_signal_invalid", RANKING_SIGNAL_VERSION)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image_hasher::ImageHash;

    use crate::features::smart_culling::types::{
        EyeDisposition, EyeResult, FaceResult, KeyPersonEvidence,
    };

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

    fn optical() -> ScoreEvidence {
        legacy_optical_evidence(100.0, 0.8)
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
            expression_score: None,
            expression_confidence: 0.0,
            expression_reason: "model_unavailable".to_string(),
            expression_descriptor: None,
            sharpness_metric: 100.0,
            sharpness_confidence: 1.0,
            exposure_metric: 0.8,
            exposure_confidence: 1.0,
            identity_embedding: None,
        };
        let mut closed = open.clone();
        closed.left_eye = eye("closed");
        closed.eye_disposition = EyeDisposition::Unusable;
        assert!(
            face_performance(&open, &optical()).score > face_performance(&closed, &optical()).score
        );
    }

    #[test]
    fn unknown_eye_interval_midpoint_stays_between_known_open_and_closed() {
        let known_open = FaceResult {
            bbox: [0.0; 4],
            landmarks: [(0.0, 0.0); 5],
            detection_score: 1.0,
            left_eye: eye("open"),
            right_eye: eye("open"),
            eye_disposition: EyeDisposition::Open,
            expression_state: "unknown".to_string(),
            expression_score: None,
            expression_confidence: 0.0,
            expression_reason: "model_unavailable".to_string(),
            expression_descriptor: None,
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

        let open_performance = face_performance(&known_open, &optical());
        let unknown_performance = face_performance(&unknown, &optical());
        let closed_performance = face_performance(&known_closed, &optical());

        assert!(open_performance.score > unknown_performance.score);
        assert!(unknown_performance.score > closed_performance.score);
        assert!(unknown_performance.confidence < open_performance.confidence);
    }

    #[test]
    fn scored_expression_outranks_unscored_technical_states_when_eyes_match() {
        let mut scored = FaceResult {
            bbox: [0.0; 4],
            landmarks: [(0.0, 0.0); 5],
            detection_score: 1.0,
            left_eye: eye("open"),
            right_eye: eye("open"),
            eye_disposition: EyeDisposition::Open,
            expression_state: "scored".to_string(),
            expression_score: Some(1.0),
            expression_confidence: 0.9,
            expression_reason: "expression_single_frame_usable_test".to_string(),
            expression_descriptor: None,
            sharpness_metric: 100.0,
            sharpness_confidence: 1.0,
            exposure_metric: 0.8,
            exposure_confidence: 1.0,
            identity_embedding: None,
        };
        let mut transitional = scored.clone();
        transitional.expression_state = "transitional".to_string();
        transitional.expression_score = None;
        transitional.expression_reason = "expression_sequence_isolated_transition".to_string();

        let scored_score = face_performance(&scored, &optical()).score;
        let transitional_score = face_performance(&transitional, &optical()).score;
        assert!(scored_score > transitional_score);

        scored.expression_state = "unknown".to_string();
        scored.expression_score = None;
        scored.expression_confidence = 0.0;
        let unknown_score = face_performance(&scored, &optical()).score;
        assert!((transitional_score - unknown_score).abs() < f64::EPSILON);
        assert!(unknown_score < scored_score);
    }

    #[test]
    fn unresolved_identity_never_receives_a_performance_rank() {
        let face = FaceResult {
            bbox: [0.0; 4],
            landmarks: [(0.0, 0.0); 5],
            detection_score: 1.0,
            left_eye: eye("open"),
            right_eye: eye("open"),
            eye_disposition: EyeDisposition::Open,
            expression_state: "stable".to_string(),
            expression_score: None,
            expression_confidence: 0.9,
            expression_reason: "expression_sequence_locally_stable".to_string(),
            expression_descriptor: None,
            sharpness_metric: 100.0,
            sharpness_confidence: 1.0,
            exposure_metric: 0.8,
            exposure_confidence: 1.0,
            identity_embedding: None,
        };
        let mut items = vec![AnalysisCandidate {
            result_id: "frame".to_string(),
            path: "frame.jpg".into(),
            member_paths: Vec::new(),
            hash: ImageHash::from_bytes(&[0; 32]).unwrap(),
            capture_time_millis: 0,
            capture_time_from_exif: true,
            sequence_number: None,
            sharpness_metric: 100.0,
            center_focus_metric: 100.0,
            exposure_metric: 0.8,
            width: 100,
            height: 100,
            faces: vec![face],
            #[cfg(all(debug_assertions, target_os = "macos"))]
            vision_quality: Default::default(),
            key_person_evidence: vec![KeyPersonEvidence {
                priority: 1,
                face_index: Some(0),
                similarity: Some(0.9),
                status: "suspected".to_string(),
                auto_score_eligible: false,
                performance_rank: None,
            }],
        }];

        rank_key_person_performance(&mut items, &[0]);

        assert_eq!(items[0].key_person_evidence[0].performance_rank, None);
    }
}
