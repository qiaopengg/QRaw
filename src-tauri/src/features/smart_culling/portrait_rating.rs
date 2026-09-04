//! Frozen discrete rating contract for portrait-mode smart culling.
//!
//! This module stays a pure policy layer. The active scorer reaches it only
//! through `portrait_rating_adapter`, which owns evidence-to-state mapping.

pub(crate) const PORTRAIT_RATING_POLICY_VERSION: &str = "qraw-portrait-rating-discrete-1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubjectClarity {
    Clear,
    Unclear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EyeRatingState {
    UnableToDetermine,
    Passed,
    Failed,
}

impl EyeRatingState {
    pub(crate) const fn increment(self) -> i8 {
        match self {
            Self::UnableToDetermine => 0,
            Self::Passed => 2,
            Self::Failed => -2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExpressionRatingState {
    UnableToDetermine,
    SevereFailure,
    NotRecommended,
    Natural,
    Excellent,
    Outstanding,
}

impl ExpressionRatingState {
    pub(crate) const fn increment(self) -> i8 {
        match self {
            Self::UnableToDetermine | Self::Natural => 0,
            Self::SevereFailure => -2,
            Self::NotRecommended => -1,
            Self::Excellent => 1,
            Self::Outstanding => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidationState {
    Passed,
    NotPassed,
    UnableToDetermine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OpticalAestheticChecks {
    pub(crate) optical: ValidationState,
    pub(crate) aesthetic_composition: ValidationState,
}

impl OpticalAestheticChecks {
    const fn increment(self) -> i8 {
        if matches!(self.optical, ValidationState::Passed)
            && matches!(self.aesthetic_composition, ValidationState::Passed)
        {
            1
        } else {
            0
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PortraitRatingInput {
    pub(crate) subject_clarity: SubjectClarity,
    pub(crate) eyes: EyeRatingState,
    pub(crate) expression: ExpressionRatingState,
    pub(crate) optical_aesthetic: OpticalAestheticChecks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PortraitRatingBreakdown {
    pub(crate) eye_increment: i8,
    pub(crate) expression_increment: i8,
    pub(crate) optical_aesthetic_increment: i8,
    pub(crate) raw_rating: i8,
    pub(crate) final_rating: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PortraitRatingDecision {
    SubjectUnclear,
    Scored(PortraitRatingBreakdown),
}

impl PortraitRatingDecision {
    pub(crate) const fn final_rating(self) -> u8 {
        match self {
            Self::SubjectUnclear => 0,
            Self::Scored(breakdown) => breakdown.final_rating,
        }
    }
}

pub(crate) fn calculate_portrait_rating(input: PortraitRatingInput) -> PortraitRatingDecision {
    if matches!(input.subject_clarity, SubjectClarity::Unclear) {
        return PortraitRatingDecision::SubjectUnclear;
    }

    let eye_increment = input.eyes.increment();
    let expression_increment = input.expression.increment();
    let optical_aesthetic_increment = input.optical_aesthetic.increment();
    let raw_rating = eye_increment + expression_increment + optical_aesthetic_increment;

    PortraitRatingDecision::Scored(PortraitRatingBreakdown {
        eye_increment,
        expression_increment,
        optical_aesthetic_increment,
        raw_rating,
        final_rating: raw_rating.clamp(0, 5) as u8,
    })
}

#[cfg(test)]
#[path = "portrait_rating_tests.rs"]
mod tests;
