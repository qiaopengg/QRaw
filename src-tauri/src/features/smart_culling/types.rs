pub(crate) const MIN_RELIABLE_FACE_DETECTION_SCORE: f32 = 0.60;

#[derive(Debug, Clone)]
pub struct EyeResult {
    pub open_probability: Option<f32>,
    pub state: String,
    pub confidence: f32,
    pub effective_pixels: u32,
    pub sharpness_metric: Option<f64>,
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
    pub expression_state: String,
    pub expression_confidence: f32,
    pub expression_reason: String,
    pub sharpness_metric: f64,
    pub sharpness_confidence: f32,
    pub exposure_metric: f64,
    pub exposure_confidence: f32,
    /// Task-only identity data. This type is deliberately not serializable.
    pub identity_embedding: Option<Vec<f32>>,
}

impl FaceResult {
    pub fn has_closed_eye(&self) -> bool {
        self.left_eye.state == "closed" || self.right_eye.state == "closed"
    }

    pub fn eye_state_is_known(&self) -> bool {
        self.left_eye.state != "unknown" && self.right_eye.state != "unknown"
    }
}
