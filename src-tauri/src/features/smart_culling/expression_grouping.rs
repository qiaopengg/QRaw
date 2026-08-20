//! Maps chronological capture groups to expression's same-subject API.
//!
//! Identity tracking and expression assessment stay separate: this module only
//! selects subjects whose continuity is already reliable. It never reads or
//! mutates eye evidence.

use std::collections::{BTreeMap, BTreeSet};

use super::expression::apply_same_subject_sequence;
use super::scoring::AnalysisCandidate;

pub(crate) fn apply_group_expression_sequences(
    items: &mut [AnalysisCandidate],
    group_indices: &[usize],
    requested_mode: &str,
) {
    if group_indices.len() < 3
        || !matches!(
            requested_mode,
            "portrait" | "group" | "environment" | "auto"
        )
    {
        return;
    }

    let priorities = group_indices
        .iter()
        .flat_map(|index| {
            items[*index]
                .key_person_evidence
                .iter()
                .map(|evidence| evidence.priority)
        })
        .collect::<BTreeSet<_>>();
    if !priorities.is_empty() {
        if !matches!(requested_mode, "portrait" | "group") {
            return;
        }
        for priority in priorities {
            let targets = group_indices
                .iter()
                .map(|index| {
                    let face_index = items[*index]
                        .key_person_evidence
                        .iter()
                        .find(|evidence| evidence.priority == priority)
                        .filter(|evidence| {
                            evidence.status == "confirmed" && evidence.auto_score_eligible
                        })
                        .and_then(|evidence| evidence.face_index);
                    (*index, face_index)
                })
                .collect();
            apply_target_sequence(items, &targets);
        }
        return;
    }

    // A single detected face per frame does not prove identity continuity:
    // an A-B-A subject switch can still occur inside one capture group. Until
    // a validated tracker confirms the same person, no-key sequences remain
    // unknown instead of joining different people's expression descriptors.
}

fn apply_target_sequence(
    items: &mut [AnalysisCandidate],
    targets: &BTreeMap<usize, Option<usize>>,
) {
    let mut frames = items
        .iter_mut()
        .enumerate()
        .filter_map(|(item_index, item)| {
            targets
                .get(&item_index)
                .map(|face_index| face_index.and_then(|face_index| item.faces.get_mut(face_index)))
        })
        .collect::<Vec<_>>();
    apply_same_subject_sequence(&mut frames);
}

#[cfg(test)]
mod tests {
    use image_hasher::ImageHash;

    use super::*;
    use crate::features::smart_culling::types::{
        EyeDisposition, EyeResult, FaceResult, KeyPersonEvidence,
    };

    fn face(x: f32) -> FaceResult {
        FaceResult {
            bbox: [x, 10.0, 40.0, 40.0],
            landmarks: [(0.0, 0.0); 5],
            detection_score: 0.95,
            left_eye: EyeResult::unavailable("test", 0, None),
            right_eye: EyeResult::unavailable("test", 0, None),
            eye_disposition: EyeDisposition::Unknown,
            expression_state: "sentinel".to_string(),
            expression_confidence: 0.0,
            expression_reason: "sentinel".to_string(),
            expression_descriptor: None,
            sharpness_metric: 100.0,
            sharpness_confidence: 1.0,
            exposure_metric: 0.8,
            exposure_confidence: 1.0,
            identity_embedding: None,
        }
    }

    fn candidate(faces: Vec<FaceResult>) -> AnalysisCandidate {
        AnalysisCandidate {
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
            faces,
            key_person_evidence: Vec::new(),
        }
    }

    #[test]
    fn one_face_per_frame_without_verified_identity_stays_unknown() {
        let mut items = vec![
            candidate(vec![face(5.0)]),
            candidate(vec![face(5.0)]),
            candidate(vec![face(5.0)]),
        ];

        apply_group_expression_sequences(&mut items, &[0, 1, 2], "portrait");

        assert!(
            items
                .iter()
                .all(|item| item.faces[0].expression_reason == "sentinel")
        );
    }

    #[test]
    fn unverified_a_b_a_subject_switch_is_never_joined() {
        let mut first = face(5.0);
        first.identity_embedding = Some(vec![1.0; 128]);
        let mut second = face(5.0);
        second.identity_embedding = Some(vec![-1.0; 128]);
        let mut third = face(5.0);
        third.identity_embedding = Some(vec![1.0; 128]);
        let mut items = vec![
            candidate(vec![first]),
            candidate(vec![second]),
            candidate(vec![third]),
        ];

        apply_group_expression_sequences(&mut items, &[0, 1, 2], "portrait");

        assert!(
            items
                .iter()
                .all(|item| item.faces[0].expression_reason == "sentinel")
        );
    }

    #[test]
    fn untracked_multiple_subjects_are_not_joined_across_frames() {
        let mut items = vec![
            candidate(vec![face(5.0), face(55.0)]),
            candidate(vec![face(5.0), face(55.0)]),
            candidate(vec![face(5.0), face(55.0)]),
        ];

        apply_group_expression_sequences(&mut items, &[0, 1, 2], "portrait");

        assert!(items.iter().all(|item| {
            item.faces
                .iter()
                .all(|face| face.expression_reason == "sentinel")
        }));
    }

    #[test]
    fn only_confirmed_key_identity_slots_are_assessed() {
        let mut items = vec![
            candidate(vec![face(5.0)]),
            candidate(vec![face(5.0)]),
            candidate(vec![face(5.0)]),
        ];
        for (index, item) in items.iter_mut().enumerate() {
            let confirmed = index != 1;
            item.key_person_evidence = vec![KeyPersonEvidence {
                priority: 1,
                face_index: Some(0),
                similarity: Some(0.95),
                status: if confirmed { "confirmed" } else { "suspected" }.to_string(),
                auto_score_eligible: confirmed,
                performance_rank: None,
            }];
        }

        apply_group_expression_sequences(&mut items, &[0, 1, 2], "group");

        assert_eq!(
            items[0].faces[0].expression_reason,
            "expression_frame_evidence_unavailable"
        );
        assert_eq!(items[1].faces[0].expression_reason, "sentinel");
        assert_eq!(
            items[2].faces[0].expression_reason,
            "expression_frame_evidence_unavailable"
        );
    }

    #[test]
    fn group_without_key_people_never_uses_face_expression() {
        let mut items = vec![
            candidate(vec![face(5.0)]),
            candidate(vec![face(5.0)]),
            candidate(vec![face(5.0)]),
        ];

        apply_group_expression_sequences(&mut items, &[0, 1, 2], "group");

        assert!(
            items
                .iter()
                .all(|item| item.faces[0].expression_reason == "sentinel")
        );
    }
}
