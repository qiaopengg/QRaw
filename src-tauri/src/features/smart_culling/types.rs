use super::expression::{ExpressionDescriptor, ExpressionSequenceAssessment};

pub(crate) const MIN_RELIABLE_FACE_DETECTION_SCORE: f32 = 0.60;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum EyeDisposition {
    Unknown,
    Open,
    Unusable,
    DeliberatePoseCandidate,
}

#[derive(Debug, Clone)]
pub struct EyeResult {
    pub open_probability: Option<f32>,
    pub state: String,
    pub confidence: f32,
    pub reason: String,
    pub effective_pixels: u32,
    pub sharpness_metric: Option<f64>,
}

impl EyeResult {
    pub fn unavailable(
        reason: impl Into<String>,
        effective_pixels: u32,
        sharpness_metric: Option<f64>,
    ) -> Self {
        Self {
            open_probability: None,
            state: "unknown".to_string(),
            confidence: 0.0,
            reason: reason.into(),
            effective_pixels,
            sharpness_metric,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeyPersonEvidence {
    pub priority: usize,
    pub face_index: Option<usize>,
    pub similarity: Option<f32>,
    pub status: String,
    pub auto_score_eligible: bool,
    pub performance_rank: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct FaceResult {
    /// Bounding box in pixels of the rendered analysis input.
    pub bbox: [f32; 4],
    /// YuNet order: right eye, left eye, nose, right mouth, left mouth.
    pub landmarks: [(f32, f32); 5],
    pub detection_score: f32,
    pub left_eye: EyeResult,
    pub right_eye: EyeResult,
    pub eye_disposition: EyeDisposition,
    pub expression_state: String,
    pub expression_score: Option<f32>,
    pub expression_confidence: f32,
    pub expression_reason: String,
    /// Task-only, read-only single-frame evidence used after chronological
    /// grouping. This type is deliberately not serializable.
    pub(in crate::features::smart_culling) expression_descriptor: Option<ExpressionDescriptor>,
    pub sharpness_metric: f64,
    pub sharpness_confidence: f32,
    pub exposure_metric: f64,
    pub exposure_confidence: f32,
    /// Task-only identity data. This type is deliberately not serializable.
    pub identity_embedding: Option<Vec<f32>>,
}

impl FaceResult {
    pub fn has_unusable_eye(&self) -> bool {
        self.eye_disposition == EyeDisposition::Unusable
    }

    pub fn eye_state_is_known(&self) -> bool {
        matches!(
            self.eye_disposition,
            EyeDisposition::Open | EyeDisposition::Unusable
        )
    }

    pub(in crate::features::smart_culling) fn expression_descriptor(
        &self,
    ) -> Option<&ExpressionDescriptor> {
        self.expression_descriptor.as_ref()
    }

    pub(in crate::features::smart_culling) fn apply_expression_sequence_assessment(
        &mut self,
        assessment: &ExpressionSequenceAssessment,
    ) {
        let evidence = assessment.as_evidence();
        self.expression_state = evidence.state.to_string();
        self.expression_score = evidence.quality_score;
        self.expression_confidence = evidence.confidence;
        self.expression_reason = evidence.reason.to_string();
    }
}
