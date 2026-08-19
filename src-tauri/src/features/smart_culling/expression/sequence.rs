//! Conservative chronological sequence policy for expression technical state.

use super::{
    EXPRESSION_SEQUENCE_POLICY_VERSION, ExpressionDescriptor, ExpressionEvidence,
};

const MIN_SEQUENCE_LENGTH: usize = 3;
const MIN_STABLE_DISTANCE: f32 = 0.035;
const MAX_STABLE_DISTANCE: f32 = 0.075;
const MIN_TRANSITION_DISTANCE: f32 = 0.120;
const MAX_TRANSITION_DISTANCE: f32 = 0.220;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::features::smart_culling) enum ExpressionTechnicalState {
    Unknown,
    Stable,
    Transitional,
}

impl ExpressionTechnicalState {
    pub(in crate::features::smart_culling) const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Stable => "stable",
            Self::Transitional => "transitional",
        }
    }
}

/// Read-only per-frame result. It remains independent from rating and emotion.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::features::smart_culling) struct ExpressionSequenceAssessment {
    state: ExpressionTechnicalState,
    confidence: f32,
    reason: &'static str,
}

impl ExpressionSequenceAssessment {
    pub(in crate::features::smart_culling) const fn state(&self) -> ExpressionTechnicalState {
        self.state
    }

    pub(in crate::features::smart_culling) const fn confidence(&self) -> f32 {
        self.confidence
    }

    pub(in crate::features::smart_culling) const fn reason(&self) -> &'static str {
        self.reason
    }

    pub(in crate::features::smart_culling) const fn policy_version(&self) -> &'static str {
        EXPRESSION_SEQUENCE_POLICY_VERSION
    }

    pub(in crate::features::smart_culling) fn as_evidence(&self) -> ExpressionEvidence {
        ExpressionEvidence::from_sequence(self)
    }

    fn unknown(reason: &'static str) -> Self {
        Self {
            state: ExpressionTechnicalState::Unknown,
            confidence: 0.0,
            reason,
        }
    }
}

/// Assesses one chronological similar-shot group for one already-tracked face.
///
/// The caller owns grouping, capture-time ordering and identity tracking. This
/// API intentionally accepts no path, filename, rating or human annotation.
pub(in crate::features::smart_culling) fn assess_sequence(
    descriptors: &[ExpressionDescriptor],
) -> Vec<ExpressionSequenceAssessment> {
    if descriptors.len() < MIN_SEQUENCE_LENGTH {
        return descriptors
            .iter()
            .map(|_| ExpressionSequenceAssessment::unknown("expression_sequence_too_short"))
            .collect();
    }

    let adjacent_distances = descriptors
        .windows(2)
        .map(|pair| comparable_distance(&pair[0], &pair[1]))
        .collect::<Vec<_>>();
    let reliable_distances = adjacent_distances
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let (stable_limit, transition_limit) = adaptive_limits(&reliable_distances);

    descriptors
        .iter()
        .enumerate()
        .map(|(index, current)| {
            if !current.is_reliable() {
                return ExpressionSequenceAssessment::unknown(
                    "expression_frame_evidence_unreliable",
                );
            }
            if index == 0 || index + 1 == descriptors.len() {
                return ExpressionSequenceAssessment::unknown("expression_sequence_boundary");
            }
            let (Some(previous_distance), Some(next_distance)) =
                (adjacent_distances[index - 1], adjacent_distances[index])
            else {
                return ExpressionSequenceAssessment::unknown(
                    "expression_neighbors_not_comparable",
                );
            };
            let previous = &descriptors[index - 1];
            let next = &descriptors[index + 1];

            if previous_distance <= stable_limit && next_distance <= stable_limit {
                let confidence = frame_reliability(previous, current, next)
                    * (1.0 - previous_distance.max(next_distance) / stable_limit)
                        .clamp(0.35, 1.0);
                return ExpressionSequenceAssessment {
                    state: ExpressionTechnicalState::Stable,
                    confidence,
                    reason: "expression_sequence_locally_stable",
                };
            }

            let anchor_distance = comparable_distance(previous, next);
            if previous_distance >= transition_limit
                && next_distance >= transition_limit
                && anchor_distance.is_some_and(|distance| distance <= stable_limit)
            {
                let anchor_distance = anchor_distance.unwrap_or(stable_limit);
                let separation = (previous_distance.min(next_distance) / transition_limit)
                    .clamp(0.35, 1.0);
                let return_to_anchor = (1.0 - anchor_distance / stable_limit).clamp(0.35, 1.0);
                return ExpressionSequenceAssessment {
                    state: ExpressionTechnicalState::Transitional,
                    confidence: frame_reliability(previous, current, next)
                        * separation
                        * return_to_anchor,
                    reason: "expression_sequence_isolated_transition",
                };
            }

            ExpressionSequenceAssessment::unknown("expression_sequence_ambiguous")
        })
        .collect()
}

fn adaptive_limits(distances: &[f32]) -> (f32, f32) {
    if distances.is_empty() {
        return (MIN_STABLE_DISTANCE, MIN_TRANSITION_DISTANCE);
    }
    let distance_median = median(distances);
    let deviations = distances
        .iter()
        .map(|distance| (distance - distance_median).abs())
        .collect::<Vec<_>>();
    let median_absolute_deviation = median(&deviations);
    (
        (distance_median + 2.5 * median_absolute_deviation)
            .clamp(MIN_STABLE_DISTANCE, MAX_STABLE_DISTANCE),
        (distance_median + 4.0 * median_absolute_deviation)
            .clamp(MIN_TRANSITION_DISTANCE, MAX_TRANSITION_DISTANCE),
    )
}

fn comparable_distance(
    left: &ExpressionDescriptor,
    right: &ExpressionDescriptor,
) -> Option<f32> {
    if !left.is_comparable_with(right) {
        return None;
    }
    let squared_sum = left
        .non_eye_blendshapes()
        .iter()
        .zip(right.non_eye_blendshapes())
        .map(|(left, right)| (left - right).powi(2))
        .sum::<f32>()
        + (left.tongue_out() - right.tongue_out()).powi(2);
    Some((squared_sum / (left.non_eye_blendshapes().len() as f32 + 1.0)).sqrt())
}

fn frame_reliability(
    previous: &ExpressionDescriptor,
    current: &ExpressionDescriptor,
    next: &ExpressionDescriptor,
) -> f32 {
    previous
        .reliability()
        .min(current.reliability())
        .min(next.reliability())
}

fn median(values: &[f32]) -> f32 {
    let mut values = values.to_vec();
    values.sort_by(f32::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) * 0.5
    } else {
        values[middle]
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::features::smart_culling::expression::{
        EXPRESSION_DESCRIPTOR_VERSION, NON_EYE_BLENDSHAPE_NAMES,
    };

    fn descriptor(changes: &[(&'static str, f32)]) -> ExpressionDescriptor {
        descriptor_with_pose(changes, 0.0, 0.0, 0.01)
    }

    fn descriptor_with_pose(
        changes: &[(&'static str, f32)],
        pitch: f32,
        yaw: f32,
        landmark_error: f32,
    ) -> ExpressionDescriptor {
        let mut scores = NON_EYE_BLENDSHAPE_NAMES
            .into_iter()
            .map(|name| (name, 0.0))
            .chain([("eyeBlinkLeft", 0.0), ("eyeBlinkRight", 0.0)])
            .collect::<BTreeMap<_, _>>();
        for &(name, value) in changes {
            scores.insert(name, value);
        }
        ExpressionDescriptor::from_face_motion(
            &scores,
            0.0,
            Some(pitch),
            Some(yaw),
            Some(landmark_error),
            0.99,
        )
        .unwrap()
    }

    #[test]
    fn sequence_policy_and_descriptor_are_independently_versioned() {
        let results = assess_sequence(&[
            descriptor(&[]),
            descriptor(&[]),
            descriptor(&[]),
        ]);

        assert_eq!(EXPRESSION_DESCRIPTOR_VERSION, "qraw-expression-descriptor-1.0");
        assert_eq!(
            results[1].policy_version(),
            "qraw-expression-sequence-policy-1.0"
        );
    }

    #[test]
    fn a_short_sequence_stays_unknown() {
        let results = assess_sequence(&[descriptor(&[]), descriptor(&[])]);

        assert!(results
            .iter()
            .all(|result| result.state() == ExpressionTechnicalState::Unknown));
        assert!(results
            .iter()
            .all(|result| result.reason() == "expression_sequence_too_short"));
    }

    #[test]
    fn unchanged_interior_frame_is_stable_but_boundaries_are_unknown() {
        let results = assess_sequence(&[
            descriptor(&[("mouthSmileLeft", 0.3)]),
            descriptor(&[("mouthSmileLeft", 0.3)]),
            descriptor(&[("mouthSmileLeft", 0.3)]),
        ]);

        assert_eq!(results[0].state(), ExpressionTechnicalState::Unknown);
        assert_eq!(results[1].state(), ExpressionTechnicalState::Stable);
        assert_eq!(results[2].state(), ExpressionTechnicalState::Unknown);
        assert!(results[1].confidence() > 0.0);
        assert_eq!(results[1].as_evidence().state, "stable");
    }

    #[test]
    fn isolated_non_eye_motion_is_transitional_without_naming_an_emotion() {
        let transition = [
            ("jawOpen", 0.9),
            ("mouthFunnel", 0.9),
            ("mouthLeft", 0.9),
            ("mouthRight", 0.9),
        ];
        let results = assess_sequence(&[
            descriptor(&[]),
            descriptor(&transition),
            descriptor(&[]),
        ]);

        assert_eq!(results[1].state(), ExpressionTechnicalState::Transitional);
        assert_eq!(
            results[1].reason(),
            "expression_sequence_isolated_transition"
        );
        assert_eq!(results[1].as_evidence().state, "transitional");
    }

    #[test]
    fn eye_only_changes_cannot_create_an_expression_transition() {
        let baseline = descriptor(&[]);
        let changed_eyes = descriptor(&[("eyeBlinkLeft", 1.0), ("eyeBlinkRight", 1.0)]);
        let results = assess_sequence(&[baseline.clone(), changed_eyes, baseline]);

        assert_eq!(results[1].state(), ExpressionTechnicalState::Stable);
    }

    #[test]
    fn large_pose_change_is_unknown_instead_of_expression_motion() {
        let results = assess_sequence(&[
            descriptor_with_pose(&[], 0.0, 0.0, 0.01),
            descriptor_with_pose(&[("jawOpen", 0.8)], 0.0, 20.0, 0.01),
            descriptor_with_pose(&[], 0.0, 0.0, 0.01),
        ]);

        assert_eq!(results[1].state(), ExpressionTechnicalState::Unknown);
        assert_eq!(
            results[1].reason(),
            "expression_neighbors_not_comparable"
        );
    }

    #[test]
    fn unreliable_landmark_geometry_stays_unknown() {
        let results = assess_sequence(&[
            descriptor(&[]),
            descriptor_with_pose(&[], 0.0, 0.0, 0.30),
            descriptor(&[]),
        ]);

        assert_eq!(results[1].state(), ExpressionTechnicalState::Unknown);
        assert_eq!(
            results[1].reason(),
            "expression_frame_evidence_unreliable"
        );
    }

    #[test]
    fn gradual_change_remains_unknown_instead_of_being_called_bad() {
        let results = assess_sequence(&[
            descriptor(&[]),
            descriptor(&[("jawOpen", 0.35), ("mouthFunnel", 0.35)]),
            descriptor(&[("jawOpen", 0.70), ("mouthFunnel", 0.70)]),
        ]);

        assert_eq!(results[1].state(), ExpressionTechnicalState::Unknown);
        assert_eq!(results[1].reason(), "expression_sequence_ambiguous");
    }
}
