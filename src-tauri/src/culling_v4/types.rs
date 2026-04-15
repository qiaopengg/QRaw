use image::{DynamicImage, GrayImage};
use ort::session::Session;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ═══════════════════════════════════════════════════════════════
// Stage 0: Asset Registry
// ═══════════════════════════════════════════════════════════════

pub struct AssetRegistry {
    pub assets: Vec<Asset>,
    pub raw_jpeg_pairs: HashMap<String, String>, // RAW path → JPEG path
}

impl AssetRegistry {
    pub fn primary_count(&self) -> usize {
        self.assets.iter().filter(|a| a.is_primary).count()
    }
}

pub struct Asset {
    pub path: String,
    pub stem: String,
    pub is_raw: bool,
    pub is_primary: bool,
    pub file_size: u64,
    pub capture_time: i64,
    pub iso: Option<u32>,
    pub exposure_time: Option<f32>,
    pub focal_length: Option<f32>,
    pub thumbnail: Arc<DynamicImage>,
    pub gray_thumbnail: Arc<GrayImage>,
}

// ═══════════════════════════════════════════════════════════════
// Stage 1: Technical Verdict
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TechnicalVerdict {
    Pass {
        sharpness: f64,
        subject_sharpness: f64,
        exposure_health: f64,
        dynamic_range: f64,
        nima_technical: Option<f64>,
    },
    Marginal {
        reason: TechnicalIssue,
        sharpness: f64,
        exposure_health: f64,
    },
    Fail {
        reason: TechnicalIssue,
    },
}

impl TechnicalVerdict {
    pub fn is_fail(&self) -> bool {
        matches!(self, TechnicalVerdict::Fail { .. })
    }

    pub fn subject_sharpness_or(&self, default: f64) -> f64 {
        match self {
            TechnicalVerdict::Pass { subject_sharpness, .. } => *subject_sharpness,
            _ => default,
        }
    }

    pub fn exposure_health_or(&self, default: f64) -> f64 {
        match self {
            TechnicalVerdict::Pass { exposure_health, .. } => *exposure_health,
            TechnicalVerdict::Marginal { exposure_health, .. } => *exposure_health,
            _ => default,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TechnicalIssue {
    SevereBlur,
    MildBlur,
    SevereOverexposure,
    SevereUnderexposure,
    MotionBlur,
}

impl TechnicalIssue {
    pub fn to_tag(&self) -> String {
        match self {
            TechnicalIssue::SevereBlur => "severeBlur".to_string(),
            TechnicalIssue::MildBlur => "mildBlur".to_string(),
            TechnicalIssue::SevereOverexposure => "severeOverexposure".to_string(),
            TechnicalIssue::SevereUnderexposure => "severeUnderexposure".to_string(),
            TechnicalIssue::MotionBlur => "motionBlur".to_string(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// Stage 2: Burst Groups
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BurstGroup {
    pub group_id: String,
    pub cover_index: usize,
    pub members: Vec<BurstMember>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BurstMember {
    pub asset_index: usize,
    pub is_cover: bool,
}

// ═══════════════════════════════════════════════════════════════
// Stage 3: Portrait Verdict
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortraitVerdict {
    pub asset_index: usize,
    pub has_faces: bool,
    pub primary_face_area_ratio: f64,
    pub faces: Vec<FaceAnalysis>,
    pub composition_score: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FaceAnalysis {
    pub bbox: (f32, f32, f32, f32),
    pub area_ratio: f64,
    pub face_aspect_ratio: f32,
    pub is_extreme_profile: bool,
    // 106-point landmark derived
    pub ear_left: f64,
    pub ear_right: f64,
    pub is_eye_closed: bool,
    pub mouth_open_ratio: f64,
    pub brow_furrow: f64,
    pub mouth_corner_down: f64,
    // Expression model
    pub smile_prob: f64,
    pub negative_emotion_prob: f64,
    pub emotion_label: String,
    // Composition
    pub is_edge_cropped: bool,
    pub headroom_ratio: f32,
}

// ═══════════════════════════════════════════════════════════════
// Stage 4: Final Rating
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalRating {
    pub path: String,
    pub stars: u8,
    pub reasons: Vec<String>,
    pub quality_score: f64,
    pub breakdown: ScoreBreakdown,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScoreBreakdown {
    pub sharpness: f64,
    pub subject_sharpness: f64,
    pub exposure: f64,
    pub dynamic_range: f64,
    pub nima_technical: Option<f64>,
    pub nima_aesthetic: Option<f64>,
    pub clip_quality: Option<f64>,
    pub face_blink: Option<bool>,
    pub face_expression: Option<f64>,
    pub face_composition: Option<f64>,
    pub composition: f64,
    pub scene_type: String,
    pub group_id: Option<String>,
    pub is_cover: bool,
}

// ═══════════════════════════════════════════════════════════════
// Scene Type
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SceneType {
    CloseUpPortrait,
    HalfBodyPortrait,
    GroupPhoto,
    Wedding,
    Landscape,
    Architecture,
    Action,
    Street,
    Default,
}

impl std::fmt::Display for SceneType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SceneType::CloseUpPortrait => write!(f, "closeUpPortrait"),
            SceneType::HalfBodyPortrait => write!(f, "halfBodyPortrait"),
            SceneType::GroupPhoto => write!(f, "groupPhoto"),
            SceneType::Wedding => write!(f, "wedding"),
            SceneType::Landscape => write!(f, "landscape"),
            SceneType::Architecture => write!(f, "architecture"),
            SceneType::Action => write!(f, "action"),
            SceneType::Street => write!(f, "street"),
            SceneType::Default => write!(f, "default"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// Model Container
// ═══════════════════════════════════════════════════════════════

pub struct CullingModelsV4 {
    pub face_detector: Mutex<Session>,
    pub yunet_detector: Option<Mutex<Session>>,
    pub landmark_106: Option<Mutex<Session>>,
    pub expression_model: Option<Mutex<Session>>, // HSEmotion or FerPlus fallback
    pub nima_aesthetic: Option<Mutex<Session>>,
    pub nima_technical: Option<Mutex<Session>>,
    // Depth Anything is NOT here — obtained from AiState.models.depth_anything
}

// ═══════════════════════════════════════════════════════════════
// Settings
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CullingSettingsV4 {
    #[serde(default = "default_blur_threshold")]
    pub blur_threshold: f64,
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: u32,
    #[serde(default = "default_ear_threshold")]
    pub ear_threshold: f64,
    #[serde(default = "default_true")]
    pub enable_nima_aesthetic: bool,
    #[serde(default = "default_true")]
    pub enable_auto_scene: bool,
    #[serde(default = "default_scene")]
    pub manual_profile: SceneType,
    #[serde(default = "default_strictness")]
    pub strictness: String, // "conservative" | "balanced" | "aggressive"
}

fn default_blur_threshold() -> f64 { 100.0 }
fn default_similarity_threshold() -> u32 { 28 }
fn default_ear_threshold() -> f64 { 0.20 }
fn default_true() -> bool { true }
fn default_scene() -> SceneType { SceneType::Default }
fn default_strictness() -> String { "balanced".to_string() }

impl Default for CullingSettingsV4 {
    fn default() -> Self {
        Self {
            blur_threshold: 100.0,
            similarity_threshold: 28,
            ear_threshold: 0.20,
            enable_nima_aesthetic: true,
            enable_auto_scene: true,
            manual_profile: SceneType::Default,
            strictness: "balanced".to_string(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// Result
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CullingResultV4 {
    pub statistics: CullingStatisticsV4,
    pub ratings: Vec<FinalRating>,
    pub burst_groups: Vec<BurstGroup>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CullingStatisticsV4 {
    pub total_analyzed: usize,
    pub total_primary: usize,
    pub rating_distribution: [usize; 6], // index 0 unused, 1-5 = star counts
    pub burst_groups_count: usize,
    pub duplicates_count: usize,
    pub technical_failures: usize,
    pub blink_detected: usize,
    pub detected_scene: String,
    pub elapsed_ms: u64,
}

// ═══════════════════════════════════════════════════════════════
// Progress
// ═══════════════════════════════════════════════════════════════

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CullingProgressV4 {
    pub current: usize,
    pub total: usize,
    pub stage: String,
}


// ═══════════════════════════════════════════════════════════════
// Legacy-compatible types for frontend (replaces culling.rs types)
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImageAnalysisResult {
    pub path: String,
    pub quality_score: f64,
    pub calibrated_score: f64,
    pub sharpness_metric: f64,
    pub center_focus_metric: f64,
    pub exposure_metric: f64,
    pub face_score: Option<f64>,
    pub aesthetic_score: Option<f64>,
    pub width: u32,
    pub height: u32,
    pub suggested_rating: u8,
    pub reasons: Vec<String>,
    pub score_breakdown: std::collections::HashMap<String, f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub face_detector_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(default)]
    pub is_cover: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyCullGroup {
    pub representative: LegacyImageAnalysisResult,
    pub duplicates: Vec<LegacyImageAnalysisResult>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LegacyCullingSuggestions {
    pub similar_groups: Vec<LegacyCullGroup>,
    pub blurry_images: Vec<LegacyImageAnalysisResult>,
    pub bad_expressions: Vec<LegacyImageAnalysisResult>,
    pub failed_paths: Vec<String>,
}
