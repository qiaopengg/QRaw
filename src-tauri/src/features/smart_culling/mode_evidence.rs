//! Adapts existing analysis output into model-independent mode evidence.
//!
//! This module owns conservative clarity gates, subject selection, and scalar
//! evidence aggregation. Mode weights, rating thresholds, and user-facing
//! reasons remain in the policy/scoring modules.

use super::mode_policy::ClarityGateTarget;
use super::quality_evidence::{
    ClarityEvidence, ClarityState, ScoreEvidence, composition_evidence_unavailable,
    legacy_optical_evidence,
};
use super::scoring::AnalysisCandidate;
use super::types::{EyeDisposition, FaceResult, MIN_RELIABLE_FACE_DETECTION_SCORE};

const MIN_SUBJECT_FRAME_AREA: f32 = 0.003;
const MIN_IMPORTANCE_TO_LARGEST_FACE: f32 = 0.20;
const MIN_WEAK_FACE_DETECTION_SCORE: f32 = 0.45;
#[cfg(all(debug_assertions, target_os = "macos"))]
const MIN_VISION_HUMAN_CONFIDENCE_FOR_REVIEW: f32 = 0.50;
#[cfg(all(debug_assertions, target_os = "macos"))]
const VISION_FACE_CAPTURE_QUALITY_CONFIDENCE_CAP: f64 = 0.49;
const DEFINITE_IMAGE_BLUR_THRESHOLD: f64 = 2.4;
const DEFINITE_CENTER_BLUR_THRESHOLD: f64 = 2.8;
const DEFINITE_PERSON_BLUR_THRESHOLD: f64 = 2.8;
const MIN_CLEAR_PERSON_DETECTION_SCORE: f32 = 0.78;
pub(super) const LEGACY_CLARITY_CONFIDENCE_CAP: f64 = 0.25;
const LEGACY_CLARITY_SOURCE_VERSION: &str = "legacy_laplacian_clarity_gate_v1";
#[cfg(all(debug_assertions, target_os = "macos"))]
const VISION_FACE_CAPTURE_QUALITY_SOURCE_VERSION: &str =
    "apple_vision_face_capture_quality_revision_3_calibration_v1";
const SIGNAL_ADAPTER_SOURCE_VERSION: &str = "mode_signal_adapter_v1";

pub(super) struct ModeEvidence {
    pub person_clarity: ScoreEvidence,
    pub eyes: ScoreEvidence,
    pub expression: ScoreEvidence,
    pub optical: ScoreEvidence,
    pub composition: ScoreEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExpressionAssessment {
    Complete,
    Unknown,
}

pub(super) fn adapt_mode_evidence(
    item: &AnalysisCandidate,
    subjects: &[&FaceResult],
) -> ModeEvidence {
    ModeEvidence {
        person_clarity: person_clarity_evidence(item, subjects),
        eyes: aggregate_eye_evidence(subjects),
        expression: aggregate_expression_evidence(subjects),
        optical: legacy_optical_evidence(item.sharpness_metric, item.exposure_metric),
        composition: composition_evidence_unavailable(),
    }
}

pub(super) fn clarity_evidence(
    target: ClarityGateTarget,
    item: &AnalysisCandidate,
    subjects: &[&FaceResult],
) -> ClarityEvidence {
    match target {
        ClarityGateTarget::Image => image_clarity_evidence(item),
        ClarityGateTarget::Person => person_gate_clarity_evidence(item, subjects),
    }
}

fn image_clarity_evidence(item: &AnalysisCandidate) -> ClarityEvidence {
    if !item.sharpness_metric.is_finite()
        || item.sharpness_metric < 0.0
        || !item.center_focus_metric.is_finite()
        || item.center_focus_metric < 0.0
    {
        return ClarityEvidence::unknown(
            "image_clarity_metric_invalid",
            LEGACY_CLARITY_SOURCE_VERSION,
        );
    }
    let score = normalize_focus(item.sharpness_metric.min(item.center_focus_metric));
    let state = if item.sharpness_metric < DEFINITE_IMAGE_BLUR_THRESHOLD
        && item.center_focus_metric < DEFINITE_CENTER_BLUR_THRESHOLD
    {
        ClarityState::Unclear
    } else {
        ClarityState::Uncertain
    };
    ClarityEvidence::try_new(
        state,
        Some(score),
        LEGACY_CLARITY_CONFIDENCE_CAP,
        if state == ClarityState::Unclear {
            "image_definitely_blurred_by_conservative_legacy_gate"
        } else {
            "image_clarity_requires_validated_model"
        },
        LEGACY_CLARITY_SOURCE_VERSION,
    )
    .expect("finite non-negative image clarity metrics must validate")
}

fn person_gate_clarity_evidence(
    item: &AnalysisCandidate,
    subjects: &[&FaceResult],
) -> ClarityEvidence {
    if subjects.is_empty()
        || subjects
            .iter()
            .any(|face| !face.sharpness_metric.is_finite() || face.sharpness_metric < 0.0)
    {
        return ClarityEvidence::unknown(
            "person_clarity_evidence_unavailable",
            LEGACY_CLARITY_SOURCE_VERSION,
        );
    }
    let weakest_focus = subjects
        .iter()
        .map(|face| face.sharpness_metric)
        .fold(f64::INFINITY, f64::min);
    let weakest_detection = subjects
        .iter()
        .map(|face| face.detection_score)
        .fold(1.0, f32::min);
    let definitely_unclear = item.sharpness_metric.is_finite()
        && item.sharpness_metric < DEFINITE_IMAGE_BLUR_THRESHOLD
        && weakest_focus < DEFINITE_PERSON_BLUR_THRESHOLD;
    let state = if definitely_unclear {
        ClarityState::Unclear
    } else if weakest_detection >= MIN_CLEAR_PERSON_DETECTION_SCORE
        && normalize_focus(weakest_focus) >= 0.55
    {
        ClarityState::Clear
    } else {
        ClarityState::Uncertain
    };
    ClarityEvidence::try_new(
        state,
        Some(normalize_focus(weakest_focus)),
        LEGACY_CLARITY_CONFIDENCE_CAP,
        match state {
            ClarityState::Clear => "person_clear_by_conservative_legacy_gate",
            ClarityState::Uncertain => "person_clarity_requires_validated_model",
            ClarityState::Unclear => "person_definitely_blurred_by_conservative_legacy_gate",
        },
        LEGACY_CLARITY_SOURCE_VERSION,
    )
    .expect("finite non-negative person clarity metrics must validate")
}

fn person_clarity_evidence(_item: &AnalysisCandidate, subjects: &[&FaceResult]) -> ScoreEvidence {
    #[cfg(all(debug_assertions, target_os = "macos"))]
    if let Some(evidence) = vision_person_clarity_evidence(_item, subjects) {
        return evidence;
    }

    legacy_person_clarity_evidence(subjects)
}

#[cfg(all(debug_assertions, target_os = "macos"))]
fn vision_person_clarity_evidence(
    item: &AnalysisCandidate,
    subjects: &[&FaceResult],
) -> Option<ScoreEvidence> {
    if subjects.is_empty() {
        return None;
    }

    let qualities = subjects
        .iter()
        .map(|subject| {
            let face_index = item
                .faces
                .iter()
                .position(|candidate| std::ptr::eq(candidate, *subject))?;
            item.vision_quality
                .face_capture_qualities
                .get(face_index)
                .copied()
                .flatten()
                .filter(|quality| quality.is_finite() && (0.0..=1.0).contains(quality))
        })
        .collect::<Option<Vec<_>>>()?;
    let weakest_quality = qualities.into_iter().fold(1.0_f32, f32::min);
    let weakest_detection = subjects
        .iter()
        .map(|face| face.detection_score)
        .fold(1.0_f32, f32::min);

    ScoreEvidence::try_available(
        f64::from(weakest_quality),
        (f64::from(weakest_detection) * VISION_FACE_CAPTURE_QUALITY_CONFIDENCE_CAP)
            .min(VISION_FACE_CAPTURE_QUALITY_CONFIDENCE_CAP),
        "apple_vision_face_capture_quality_calibration",
        VISION_FACE_CAPTURE_QUALITY_SOURCE_VERSION,
    )
    .ok()
}

fn legacy_person_clarity_evidence(subjects: &[&FaceResult]) -> ScoreEvidence {
    if subjects.is_empty()
        || subjects.iter().any(|face| {
            face.detection_score < MIN_RELIABLE_FACE_DETECTION_SCORE
                || !face.sharpness_metric.is_finite()
                || face.sharpness_metric < 0.0
        })
    {
        return ScoreEvidence::unavailable(
            "person_clarity_evidence_unavailable",
            LEGACY_CLARITY_SOURCE_VERSION,
        );
    }
    let score = subjects
        .iter()
        .map(|face| normalize_focus(face.sharpness_metric))
        .fold(1.0, f64::min);
    let confidence = subjects
        .iter()
        .map(|face| face.sharpness_confidence as f64)
        .fold(1.0, f64::min)
        .min(LEGACY_CLARITY_CONFIDENCE_CAP);
    ScoreEvidence::try_available(
        score,
        confidence,
        "legacy_face_sharpness_proxy_low_confidence",
        LEGACY_CLARITY_SOURCE_VERSION,
    )
    .unwrap_or_else(|_| {
        ScoreEvidence::unavailable(
            "person_clarity_evidence_invalid",
            LEGACY_CLARITY_SOURCE_VERSION,
        )
    })
}

fn aggregate_eye_evidence(subjects: &[&FaceResult]) -> ScoreEvidence {
    aggregate_subject_signal(subjects, eye_signal, "eye_evidence_incomplete")
}

pub(super) fn eye_signal(face: &FaceResult) -> Option<(f64, f64)> {
    match face.eye_disposition {
        EyeDisposition::Open => Some((
            1.0,
            face.left_eye.confidence.min(face.right_eye.confidence) as f64,
        )),
        EyeDisposition::Unusable => Some((
            0.0,
            face.left_eye.confidence.max(face.right_eye.confidence) as f64,
        )),
        EyeDisposition::DeliberatePoseCandidate => Some((0.65, 0.40)),
        EyeDisposition::Unknown => None,
    }
}

fn aggregate_expression_evidence(subjects: &[&FaceResult]) -> ScoreEvidence {
    aggregate_subject_signal(
        subjects,
        expression_signal,
        "expression_quality_evidence_incomplete",
    )
}

pub(super) fn expression_signal(face: &FaceResult) -> Option<(f64, f64)> {
    face.expression_score
        .filter(|score| score.is_finite() && (0.0..=1.0).contains(score))
        .filter(|_| face.expression_confidence > 0.0)
        .map(|score| (f64::from(score), f64::from(face.expression_confidence)))
}

pub(super) fn expression_assessment(subjects: &[&FaceResult]) -> ExpressionAssessment {
    if !subjects.is_empty()
        && subjects
            .iter()
            .all(|face| expression_signal(face).is_some())
    {
        ExpressionAssessment::Complete
    } else {
        ExpressionAssessment::Unknown
    }
}

fn aggregate_subject_signal(
    subjects: &[&FaceResult],
    signal: impl Fn(&FaceResult) -> Option<(f64, f64)>,
    unavailable_reason: &'static str,
) -> ScoreEvidence {
    if subjects.is_empty() {
        return ScoreEvidence::unavailable(unavailable_reason, SIGNAL_ADAPTER_SOURCE_VERSION);
    }
    let Some(values) = subjects
        .iter()
        .map(|face| signal(face))
        .collect::<Option<Vec<_>>>()
    else {
        return ScoreEvidence::unavailable(unavailable_reason, SIGNAL_ADAPTER_SOURCE_VERSION);
    };
    let score = values.iter().map(|(score, _)| *score).fold(1.0, f64::min);
    let confidence = values
        .iter()
        .map(|(_, confidence)| *confidence)
        .fold(1.0, f64::min);
    ScoreEvidence::try_available(
        score,
        confidence,
        "weakest_selected_subject_signal",
        SIGNAL_ADAPTER_SOURCE_VERSION,
    )
    .unwrap_or_else(|_| {
        ScoreEvidence::unavailable(unavailable_reason, SIGNAL_ADAPTER_SOURCE_VERSION)
    })
}

pub(super) fn important_face_indices(item: &AnalysisCandidate) -> Vec<usize> {
    let reliable = item
        .faces
        .iter()
        .enumerate()
        .filter(|face| {
            let face = face.1;
            face.detection_score >= MIN_RELIABLE_FACE_DETECTION_SCORE
                && face_area(face) / frame_area(item) >= MIN_SUBJECT_FRAME_AREA
        })
        .collect::<Vec<_>>();
    let largest = reliable
        .iter()
        .map(|(_, face)| face_area(face))
        .fold(0.0, f32::max);
    reliable
        .into_iter()
        .filter(|(_, face)| face_area(face) >= largest * MIN_IMPORTANCE_TO_LARGEST_FACE)
        .map(|(index, _)| index)
        .collect()
}

pub(super) fn important_faces(item: &AnalysisCandidate) -> Vec<&FaceResult> {
    important_face_indices(item)
        .into_iter()
        .map(|index| &item.faces[index])
        .collect()
}

pub(super) fn has_weak_person_evidence(item: &AnalysisCandidate) -> bool {
    let weak_face = item.faces.iter().any(|face| {
        face.detection_score >= MIN_WEAK_FACE_DETECTION_SCORE
            && face_area(face) / frame_area(item) >= MIN_SUBJECT_FRAME_AREA * 0.5
    });
    #[cfg(all(debug_assertions, target_os = "macos"))]
    let vision_human = item.vision_quality.human_count > 0
        && item
            .vision_quality
            .max_human_confidence
            .is_some_and(|confidence| confidence >= MIN_VISION_HUMAN_CONFIDENCE_FOR_REVIEW);
    #[cfg(not(all(debug_assertions, target_os = "macos")))]
    let vision_human = false;
    weak_face || vision_human
}

fn frame_area(item: &AnalysisCandidate) -> f32 {
    item.width.max(1) as f32 * item.height.max(1) as f32
}

pub(super) fn face_area(face: &FaceResult) -> f32 {
    face.bbox[2].max(0.0) * face.bbox[3].max(0.0)
}

pub(super) fn weakest_eye_disposition(subjects: &[&FaceResult]) -> Option<EyeDisposition> {
    if subjects
        .iter()
        .any(|face| face.eye_disposition == EyeDisposition::Unusable)
    {
        Some(EyeDisposition::Unusable)
    } else if subjects
        .iter()
        .any(|face| face.eye_disposition == EyeDisposition::Unknown)
    {
        Some(EyeDisposition::Unknown)
    } else if subjects
        .iter()
        .any(|face| face.eye_disposition == EyeDisposition::DeliberatePoseCandidate)
    {
        Some(EyeDisposition::DeliberatePoseCandidate)
    } else if subjects
        .iter()
        .all(|face| face.eye_disposition == EyeDisposition::Open)
        && !subjects.is_empty()
    {
        Some(EyeDisposition::Open)
    } else {
        None
    }
}

pub(crate) fn normalize_focus(value: f64) -> f64 {
    ((value + 1.0).log10() / 3.5).clamp(0.0, 1.0)
}
