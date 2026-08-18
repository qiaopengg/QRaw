use super::scoring::AnalysisCandidate;
use super::types::{EyeDisposition, FaceResult, MIN_RELIABLE_FACE_DETECTION_SCORE};

const MIN_SUBJECT_FRAME_AREA: f32 = 0.003;
const MIN_IMPORTANCE_TO_LARGEST_FACE: f32 = 0.20;

#[derive(Clone, Debug)]
pub(crate) struct ModeEvaluation {
    pub resolved_mode: String,
    pub score: f64,
    pub confidence: f32,
    pub requires_human_review: bool,
    pub reason_code: &'static str,
}

#[derive(Clone, Copy)]
pub(crate) struct Signal {
    value: Option<f64>,
    confidence: f64,
    unavailable_reason: Option<&'static str>,
}

impl Signal {
    pub(crate) fn available(value: f64, confidence: f64) -> Self {
        Self {
            value: Some(value.clamp(0.0, 1.0)),
            confidence: confidence.clamp(0.0, 1.0),
            unavailable_reason: None,
        }
    }

    pub(crate) fn unavailable(reason: &'static str) -> Self {
        Self {
            value: None,
            confidence: 0.0,
            unavailable_reason: Some(reason),
        }
    }
}

pub(crate) fn evaluate_mode(requested: &str, item: &AnalysisCandidate) -> ModeEvaluation {
    if requested != "auto" {
        return evaluate_explicit_mode(requested, item);
    }
    match important_faces(item).len() {
        0 => {
            let mut evaluation = landscape_evaluation(item);
            evaluation.requires_human_review = true;
            evaluation.reason_code = "auto_people_unknown";
            evaluation
        }
        1 => portrait_evaluation(item),
        _ => group_evaluation(item),
    }
}

pub(crate) fn rating_for_mode(mode: &str, score: f64) -> u8 {
    let [two, three, four, five] = match mode {
        "portrait" => [0.32, 0.48, 0.63, 0.79],
        "environment" => [0.34, 0.50, 0.65, 0.80],
        "group" => [0.30, 0.46, 0.62, 0.78],
        _ => [0.34, 0.50, 0.64, 0.78],
    };
    if score >= five {
        5
    } else if score >= four {
        4
    } else if score >= three {
        3
    } else if score >= two {
        2
    } else {
        1
    }
}

pub(crate) fn normalize_focus(value: f64) -> f64 {
    ((value + 1.0).log10() / 3.5).clamp(0.0, 1.0)
}

fn evaluate_explicit_mode(mode: &str, item: &AnalysisCandidate) -> ModeEvaluation {
    match mode {
        "portrait" => portrait_evaluation(item),
        "environment" => environment_evaluation(item),
        "group" => group_evaluation(item),
        _ => landscape_evaluation(item),
    }
}

fn landscape_evaluation(item: &AnalysisCandidate) -> ModeEvaluation {
    let (score, confidence) = combine(&[
        (
            Signal::available(normalize_focus(item.sharpness_metric), 0.90),
            0.45,
        ),
        (
            Signal::available(normalize_focus(item.center_focus_metric), 0.82),
            0.20,
        ),
        (
            Signal::available(
                item.exposure_metric,
                exposure_confidence(item.exposure_metric),
            ),
            0.35,
        ),
    ]);
    let reason_code = if item.exposure_metric >= 0.72 {
        "landscape_exposure_balanced"
    } else if normalize_focus(item.sharpness_metric) >= 0.70 {
        "landscape_detail_strong"
    } else {
        "landscape_detail_review"
    };
    ModeEvaluation {
        resolved_mode: "landscape".to_string(),
        score,
        confidence,
        requires_human_review: false,
        reason_code,
    }
}

fn portrait_evaluation(item: &AnalysisCandidate) -> ModeEvaluation {
    let faces = important_faces(item);
    let Some(subject) = faces
        .iter()
        .copied()
        .max_by(|left, right| face_area(left).total_cmp(&face_area(right)))
    else {
        return ModeEvaluation {
            resolved_mode: "portrait".to_string(),
            score: 0.0,
            confidence: 0.20,
            requires_human_review: true,
            reason_code: "portrait_subject_unreliable",
        };
    };
    let eye = eye_signal(subject);
    let (mut score, confidence) = combine(&[
        (
            Signal::available(
                normalize_focus(subject.sharpness_metric),
                subject.sharpness_confidence as f64,
            ),
            0.35,
        ),
        (eye, 0.30),
        (
            Signal::available(normalize_focus(item.sharpness_metric), 0.90),
            0.15,
        ),
        (
            Signal::available(subject.exposure_metric, subject.exposure_confidence as f64),
            0.20,
        ),
        (Signal::unavailable("expression_model_unavailable"), 0.15),
    ]);
    let quality_gate = calibration_portrait_quality_gate(item, subject);
    if let Some((_, score_cap)) = quality_gate {
        score = score.min(score_cap);
    }
    if subject.has_unusable_eye() {
        score = score.min(0.31);
    }
    if subject.eye_disposition == EyeDisposition::DeliberatePoseCandidate {
        // A single frame cannot prove intent. Preserve a potentially deliberate
        // closed-eye pose as usable, but never promote it beyond the user's
        // manually defined "basically usable" tier.
        score = score.min(0.62);
    }
    let reason_code = if subject.has_unusable_eye() {
        "portrait_closed_eyes"
    } else if let Some((reason, _)) = quality_gate {
        reason
    } else if subject.eye_disposition == EyeDisposition::DeliberatePoseCandidate {
        "portrait_deliberate_eye_pose"
    } else if subject.eye_state_is_known() {
        "portrait_eyes_open"
    } else {
        "portrait_eye_review"
    };
    ModeEvaluation {
        resolved_mode: "portrait".to_string(),
        score,
        confidence,
        requires_human_review: eye.value.is_none() && rating_for_mode("portrait", score) >= 4,
        reason_code,
    }
}

fn calibration_portrait_quality_gate(
    item: &AnalysisCandidate,
    subject: &FaceResult,
) -> Option<(&'static str, f64)> {
    #[cfg(all(debug_assertions, target_os = "macos"))]
    {
        if item.sharpness_metric < 2.4 && subject.sharpness_metric < 2.8 {
            return Some(("portrait_severe_blur", 0.31));
        }
        if item.exposure_metric < 0.10 && subject.exposure_metric < 0.70 {
            return Some(("portrait_severe_exposure", 0.31));
        }
        if subject.detection_score < 0.78 {
            return Some(("portrait_face_evidence_weak", 0.47));
        }
    }
    let _ = (item, subject);
    None
}

fn group_evaluation(item: &AnalysisCandidate) -> ModeEvaluation {
    let faces = important_faces(item);
    if faces.len() < 2 {
        return ModeEvaluation {
            resolved_mode: "group".to_string(),
            score: 0.0,
            confidence: 0.20,
            requires_human_review: true,
            reason_code: "group_subject_unreliable",
        };
    }
    let weakest_sharpness = faces
        .iter()
        .map(|face| normalize_focus(face.sharpness_metric))
        .fold(1.0, f64::min);
    let weakest_sharpness_confidence = faces
        .iter()
        .map(|face| face.sharpness_confidence as f64)
        .fold(1.0, f64::min);
    let eyes = if faces.iter().all(|face| face.eye_state_is_known()) {
        Signal::available(
            if faces.iter().any(|face| face.has_unusable_eye()) {
                0.0
            } else {
                1.0
            },
            faces
                .iter()
                .flat_map(|face| [&face.left_eye, &face.right_eye])
                .map(|eye| eye.confidence as f64)
                .fold(1.0, f64::min),
        )
    } else {
        Signal::unavailable("group_eye_evidence_incomplete")
    };
    let weakest_exposure = faces
        .iter()
        .map(|face| face.exposure_metric.clamp(0.0, 1.0))
        .fold(1.0, f64::min);
    let (mut score, confidence) = combine(&[
        (
            Signal::available(weakest_sharpness, weakest_sharpness_confidence),
            0.34,
        ),
        (eyes, 0.36),
        (Signal::available(weakest_exposure, 0.75), 0.18),
        (
            Signal::available(normalize_focus(item.sharpness_metric), 0.90),
            0.12,
        ),
        (Signal::unavailable("expression_model_unavailable"), 0.15),
    ]);
    if faces.iter().any(|face| face.has_unusable_eye()) {
        score = score.min(0.29);
    }
    let reason_code = if faces.iter().any(|face| face.has_unusable_eye()) {
        "group_closed_eyes"
    } else if faces.iter().all(|face| face.eye_state_is_known()) {
        "group_eyes_open"
    } else {
        "group_eye_review"
    };
    ModeEvaluation {
        resolved_mode: "group".to_string(),
        score,
        confidence,
        requires_human_review: eyes.value.is_none() && rating_for_mode("group", score) >= 4,
        reason_code,
    }
}

fn environment_evaluation(item: &AnalysisCandidate) -> ModeEvaluation {
    if important_faces(item).is_empty() {
        let mut landscape = landscape_evaluation(item);
        let uncertain_person_signal = item
            .faces
            .iter()
            .any(|face| face.detection_score >= MIN_RELIABLE_FACE_DETECTION_SCORE * 0.75);
        landscape.requires_human_review =
            uncertain_person_signal && rating_for_mode("landscape", landscape.score) >= 4;
        landscape.reason_code = if uncertain_person_signal {
            "environment_people_uncertain"
        } else {
            "environment_landscape_fallback"
        };
        return landscape;
    }
    let portrait = portrait_evaluation(item);
    let landscape = landscape_evaluation(item);
    let score = (portrait.score * 0.55 + landscape.score * 0.45).clamp(0.0, 1.0);
    let confidence = (portrait.confidence * 0.55 + landscape.confidence * 0.45).clamp(0.0, 1.0);
    ModeEvaluation {
        resolved_mode: "environment".to_string(),
        score,
        confidence,
        requires_human_review: portrait.requires_human_review
            && rating_for_mode("environment", score) >= 4,
        reason_code: if portrait.reason_code == "portrait_closed_eyes" {
            "environment_closed_eyes"
        } else if item.exposure_metric >= 0.72 {
            "environment_people_exposure"
        } else {
            "environment_balance_review"
        },
    }
}

fn important_faces(item: &AnalysisCandidate) -> Vec<&FaceResult> {
    let reliable = item
        .faces
        .iter()
        .filter(|face| {
            face.detection_score >= MIN_RELIABLE_FACE_DETECTION_SCORE
                && face_area(face) / frame_area(item) >= MIN_SUBJECT_FRAME_AREA
        })
        .collect::<Vec<_>>();
    let largest = reliable
        .iter()
        .map(|face| face_area(face))
        .fold(0.0, f32::max);
    reliable
        .into_iter()
        .filter(|face| face_area(face) >= largest * MIN_IMPORTANCE_TO_LARGEST_FACE)
        .collect()
}

fn frame_area(item: &AnalysisCandidate) -> f32 {
    item.width.max(1) as f32 * item.height.max(1) as f32
}

fn face_area(face: &FaceResult) -> f32 {
    face.bbox[2].max(0.0) * face.bbox[3].max(0.0)
}

pub(super) fn eye_signal(face: &FaceResult) -> Signal {
    match face.eye_disposition {
        EyeDisposition::Open => Signal::available(
            1.0,
            face.left_eye.confidence.min(face.right_eye.confidence) as f64,
        ),
        EyeDisposition::Unusable => Signal::available(
            0.0,
            face.left_eye.confidence.max(face.right_eye.confidence) as f64,
        ),
        // Residual aperture + downward pose can preserve an intentional pose
        // as usable, but confidence stays low and portrait scoring caps it at 3.
        EyeDisposition::DeliberatePoseCandidate => Signal::available(0.65, 0.40),
        EyeDisposition::Unknown => Signal::unavailable("eye_state_unknown"),
    }
}

fn exposure_confidence(exposure: f64) -> f64 {
    if exposure.is_finite() { 0.85 } else { 0.0 }
}

pub(crate) fn combine(signals: &[(Signal, f64)]) -> (f64, f32) {
    debug_assert!(
        signals
            .iter()
            .all(|(signal, _)| signal.value.is_none() == signal.unavailable_reason.is_some())
    );
    let available_weight = signals
        .iter()
        .filter(|(signal, _)| signal.value.is_some())
        .map(|(_, weight)| *weight)
        .sum::<f64>();
    if available_weight <= f64::EPSILON {
        return (0.0, 0.0);
    }
    let score = signals
        .iter()
        .filter_map(|(signal, weight)| signal.value.map(|value| value * weight))
        .sum::<f64>()
        / available_weight;
    let total_weight = signals.iter().map(|(_, weight)| *weight).sum::<f64>();
    let confidence = signals
        .iter()
        .map(|(signal, weight)| signal.confidence * weight)
        .sum::<f64>()
        / total_weight.max(f64::EPSILON);
    (score.clamp(0.0, 1.0), confidence.clamp(0.0, 1.0) as f32)
}

#[cfg(test)]
mod tests {
    use image_hasher::ImageHash;

    use super::*;
    use crate::features::smart_culling::types::EyeResult;

    fn eye(state: &str) -> EyeResult {
        EyeResult {
            open_probability: None,
            state: state.to_string(),
            confidence: if state == "unknown" { 0.0 } else { 0.9 },
            reason: format!("eye_{state}_test"),
            effective_pixels: 100,
            sharpness_metric: Some(100.0),
        }
    }

    fn face(x: f32, state: &str) -> FaceResult {
        FaceResult {
            bbox: [x, 10.0, 30.0, 30.0],
            landmarks: [(0.0, 0.0); 5],
            detection_score: 0.95,
            left_eye: eye(state),
            right_eye: eye(state),
            eye_disposition: match state {
                "open" => EyeDisposition::Open,
                "closed" => EyeDisposition::Unusable,
                "deliberate" => EyeDisposition::DeliberatePoseCandidate,
                _ => EyeDisposition::Unknown,
            },
            expression_state: "unknown".to_string(),
            expression_confidence: 0.0,
            expression_reason: "model_unavailable".to_string(),
            sharpness_metric: 300.0,
            sharpness_confidence: 0.9,
            exposure_metric: 0.8,
            exposure_confidence: 0.9,
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
            sharpness_metric: 300.0,
            center_focus_metric: 250.0,
            exposure_metric: 0.8,
            width: 100,
            height: 100,
            faces,
            key_person_evidence: Vec::new(),
        }
    }

    #[test]
    fn auto_uses_group_strategy_for_two_important_people() {
        let evaluation = evaluate_mode(
            "auto",
            &candidate(vec![face(5.0, "open"), face(55.0, "open")]),
        );
        assert_eq!(evaluation.resolved_mode, "group");
        assert!(!evaluation.requires_human_review);
    }

    #[test]
    fn portrait_without_a_reliable_subject_never_gets_an_automatic_rating() {
        let evaluation = evaluate_mode("portrait", &candidate(Vec::new()));
        assert!(evaluation.requires_human_review);
        assert_eq!(evaluation.reason_code, "portrait_subject_unreliable");
    }

    #[test]
    fn environmental_portrait_without_a_face_uses_only_landscape_evidence() {
        let item = candidate(Vec::new());
        let environment = evaluate_mode("environment", &item);
        let landscape = evaluate_mode("landscape", &item);
        assert_eq!(environment.resolved_mode, "landscape");
        assert_eq!(environment.score, landscape.score);
    }

    #[test]
    fn uncertain_environment_person_evidence_blocks_an_automatic_high_rating() {
        let mut uncertain = face(5.0, "open");
        uncertain.detection_score = 0.50;

        let evaluation = evaluate_mode("environment", &candidate(vec![uncertain]));

        assert_eq!(evaluation.reason_code, "environment_people_uncertain");
        assert!(evaluation.requires_human_review);
    }

    #[test]
    fn landscape_score_is_not_changed_by_background_eye_state() {
        let open = evaluate_mode("landscape", &candidate(vec![face(5.0, "open")]));
        let closed = evaluate_mode("landscape", &candidate(vec![face(5.0, "closed")]));
        assert_eq!(open.score, closed.score);
    }

    #[test]
    fn missing_signals_are_omitted_instead_of_becoming_middle_scores() {
        let (score, confidence) = combine(&[
            (Signal::available(1.0, 1.0), 0.5),
            (Signal::unavailable("test_signal_missing"), 0.5),
        ]);
        assert_eq!(score, 1.0);
        assert_eq!(confidence, 0.5);
    }

    #[cfg(all(debug_assertions, target_os = "macos"))]
    #[test]
    fn calibration_gate_caps_joint_global_and_face_blur() {
        let mut item = candidate(vec![face(5.0, "open")]);
        item.sharpness_metric = 2.0;
        item.faces[0].sharpness_metric = 2.0;

        let evaluation = evaluate_mode("portrait", &item);

        assert!(evaluation.score <= 0.31);
        assert_eq!(evaluation.reason_code, "portrait_severe_blur");
    }

    #[test]
    fn deliberate_closed_eye_pose_never_exceeds_three_stars() {
        let evaluation = evaluate_mode("portrait", &candidate(vec![face(5.0, "deliberate")]));

        assert_eq!(evaluation.reason_code, "portrait_deliberate_eye_pose");
        assert_eq!(rating_for_mode("portrait", evaluation.score), 3);
        assert!(!evaluation.requires_human_review);
    }
}
