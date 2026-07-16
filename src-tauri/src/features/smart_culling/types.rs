#[derive(Serialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct FaceResult {
    pub bbox: [f32; 4],
    pub eye_open_prob: Option<f32>,
    pub is_closed: bool,
}
use serde::Serialize;
