//! Immutable mode-to-signal policy for smart culling.
//!
//! This module owns only product policy: which evidence participates in a
//! shooting mode and its fixed weight. Model inference and score calibration
//! live elsewhere so they cannot silently change these contracts.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModeStrategy {
    Portrait,
    GroupWithKeyPeople,
    GroupScene,
    Environment,
    Landscape,
    AutoPeople,
    AutoScene,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClarityGateTarget {
    Image,
    Person,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SignalWeights {
    pub person_clarity: f64,
    pub eyes: f64,
    pub expression: f64,
    pub optical: f64,
    pub composition: f64,
}

impl SignalWeights {
    pub(crate) const fn total(self) -> f64 {
        self.person_clarity + self.eyes + self.expression + self.optical + self.composition
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ModePolicy {
    pub strategy: ModeStrategy,
    pub clarity_gate: ClarityGateTarget,
    pub weights: SignalWeights,
    pub closed_eye_hard_cap: bool,
}

impl ModePolicy {
    pub(crate) const fn resolved_mode(self) -> &'static str {
        match self.strategy {
            ModeStrategy::Portrait => "portrait",
            ModeStrategy::GroupWithKeyPeople | ModeStrategy::GroupScene => "group",
            ModeStrategy::Environment => "environment",
            ModeStrategy::Landscape => "landscape",
            ModeStrategy::AutoPeople | ModeStrategy::AutoScene => "auto",
        }
    }
}

pub(crate) const fn policy_for(strategy: ModeStrategy) -> ModePolicy {
    match strategy {
        ModeStrategy::Portrait => ModePolicy {
            strategy,
            clarity_gate: ClarityGateTarget::Person,
            weights: SignalWeights {
                person_clarity: 0.0,
                eyes: 0.40,
                expression: 0.40,
                optical: 0.10,
                composition: 0.10,
            },
            closed_eye_hard_cap: false,
        },
        ModeStrategy::GroupWithKeyPeople => ModePolicy {
            strategy,
            clarity_gate: ClarityGateTarget::Person,
            weights: SignalWeights {
                person_clarity: 0.0,
                eyes: 0.40,
                expression: 0.40,
                optical: 0.10,
                composition: 0.10,
            },
            closed_eye_hard_cap: true,
        },
        ModeStrategy::GroupScene => ModePolicy {
            strategy,
            clarity_gate: ClarityGateTarget::Image,
            weights: SignalWeights {
                person_clarity: 0.0,
                eyes: 0.0,
                expression: 0.0,
                optical: 0.90,
                composition: 0.10,
            },
            closed_eye_hard_cap: false,
        },
        ModeStrategy::Environment => ModePolicy {
            strategy,
            clarity_gate: ClarityGateTarget::Image,
            weights: SignalWeights {
                person_clarity: 0.05,
                eyes: 0.05,
                expression: 0.05,
                optical: 0.50,
                composition: 0.35,
            },
            closed_eye_hard_cap: false,
        },
        ModeStrategy::Landscape | ModeStrategy::AutoScene => ModePolicy {
            strategy,
            clarity_gate: ClarityGateTarget::Image,
            weights: SignalWeights {
                person_clarity: 0.0,
                eyes: 0.0,
                expression: 0.0,
                optical: 0.50,
                composition: 0.50,
            },
            closed_eye_hard_cap: false,
        },
        ModeStrategy::AutoPeople => ModePolicy {
            strategy,
            clarity_gate: ClarityGateTarget::Image,
            weights: SignalWeights {
                person_clarity: 0.50,
                eyes: 0.15,
                expression: 0.15,
                optical: 0.10,
                composition: 0.10,
            },
            closed_eye_hard_cap: false,
        },
    }
}

pub(crate) fn resolve_strategy(
    requested_mode: &str,
    has_people: bool,
    has_key_people: bool,
) -> ModeStrategy {
    match requested_mode {
        "portrait" => ModeStrategy::Portrait,
        "group" if has_key_people => ModeStrategy::GroupWithKeyPeople,
        "group" => ModeStrategy::GroupScene,
        "environment" => ModeStrategy::Environment,
        "auto" if has_people => ModeStrategy::AutoPeople,
        "auto" => ModeStrategy::AutoScene,
        _ => ModeStrategy::Landscape,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_strategy_has_exactly_one_hundred_percent_weight() {
        let strategies = [
            ModeStrategy::Portrait,
            ModeStrategy::GroupWithKeyPeople,
            ModeStrategy::GroupScene,
            ModeStrategy::Environment,
            ModeStrategy::Landscape,
            ModeStrategy::AutoPeople,
            ModeStrategy::AutoScene,
        ];

        for strategy in strategies {
            assert!((policy_for(strategy).weights.total() - 1.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn every_strategy_keeps_its_exact_frozen_weights_and_clarity_gate() {
        let expected = [
            (
                ModeStrategy::Portrait,
                ClarityGateTarget::Person,
                [0.0, 0.40, 0.40, 0.10, 0.10],
            ),
            (
                ModeStrategy::GroupWithKeyPeople,
                ClarityGateTarget::Person,
                [0.0, 0.40, 0.40, 0.10, 0.10],
            ),
            (
                ModeStrategy::GroupScene,
                ClarityGateTarget::Image,
                [0.0, 0.0, 0.0, 0.90, 0.10],
            ),
            (
                ModeStrategy::Environment,
                ClarityGateTarget::Image,
                [0.05, 0.05, 0.05, 0.50, 0.35],
            ),
            (
                ModeStrategy::Landscape,
                ClarityGateTarget::Image,
                [0.0, 0.0, 0.0, 0.50, 0.50],
            ),
            (
                ModeStrategy::AutoPeople,
                ClarityGateTarget::Image,
                [0.50, 0.15, 0.15, 0.10, 0.10],
            ),
            (
                ModeStrategy::AutoScene,
                ClarityGateTarget::Image,
                [0.0, 0.0, 0.0, 0.50, 0.50],
            ),
        ];

        for (strategy, clarity_gate, weights) in expected {
            let policy = policy_for(strategy);
            assert_eq!(policy.clarity_gate, clarity_gate);
            assert_eq!(
                [
                    policy.weights.person_clarity,
                    policy.weights.eyes,
                    policy.weights.expression,
                    policy.weights.optical,
                    policy.weights.composition,
                ],
                weights
            );
        }
    }

    #[test]
    fn only_key_person_group_keeps_the_legacy_closed_eye_hard_cap() {
        let capped = [ModeStrategy::GroupWithKeyPeople];
        let weighted_only = [
            ModeStrategy::Portrait,
            ModeStrategy::GroupScene,
            ModeStrategy::Environment,
            ModeStrategy::Landscape,
            ModeStrategy::AutoPeople,
            ModeStrategy::AutoScene,
        ];

        assert!(
            capped
                .into_iter()
                .all(|strategy| policy_for(strategy).closed_eye_hard_cap)
        );
        assert!(
            weighted_only
                .into_iter()
                .all(|strategy| !policy_for(strategy).closed_eye_hard_cap)
        );
    }

    #[test]
    fn group_strategy_changes_only_when_key_people_are_selected() {
        assert_eq!(
            resolve_strategy("group", true, true),
            ModeStrategy::GroupWithKeyPeople
        );
        assert_eq!(
            resolve_strategy("group", true, false),
            ModeStrategy::GroupScene
        );
    }

    #[test]
    fn auto_uses_people_presence_not_people_count() {
        assert_eq!(
            resolve_strategy("auto", true, false),
            ModeStrategy::AutoPeople
        );
        assert_eq!(
            resolve_strategy("auto", false, false),
            ModeStrategy::AutoScene
        );
    }
}
