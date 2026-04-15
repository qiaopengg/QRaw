use serde::{Deserialize, Serialize};

use crate::file_management::parse_virtual_path;
use crate::image_processing::ImageMetadata;

pub const CACHE_VERSION: u32 = 4;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CullingCacheV4 {
    pub version: u32,
    pub file_hash: String,
    pub file_size: u64,
    // Stage 1 results
    pub sharpness: f64,
    pub subject_sharpness: f64,
    pub exposure_health: f64,
    pub dynamic_range: f64,
    pub nima_technical: Option<f64>,
    pub verdict: String, // "pass" | "marginal" | "fail"
    // Stage 3 results
    pub face_count: u32,
    pub has_blink: bool,
    pub ear_min: Option<f64>,
    pub smile_avg: Option<f64>,
    pub composition_score: Option<f64>,
    // EXIF
    pub capture_time: i64,
}

/// Check if a valid cache exists for the given file path
pub fn check_cache(path: &str) -> Option<CullingCacheV4> {
    let (_, sidecar_path) = parse_virtual_path(path);
    let content = std::fs::read_to_string(&sidecar_path).ok()?;
    let metadata: ImageMetadata = serde_json::from_str(&content).ok()?;

    let culling_data = metadata.adjustments.get("aiCulling")?;
    let cache_val = culling_data.get("cache")?;
    let cache: CullingCacheV4 = serde_json::from_value(cache_val.clone()).ok()?;

    if cache.version != CACHE_VERSION {
        return None;
    }

    // Verify file fingerprint (first 64KB)
    let file_bytes = std::fs::read(path).ok()?;
    let hash_input = &file_bytes[..file_bytes.len().min(65536)];
    let hash = blake3::hash(hash_input).to_hex().to_string();

    if cache.file_hash != hash {
        return None;
    }

    Some(cache)
}

/// Compute file fingerprint for cache key
pub fn compute_file_hash(path: &str) -> Option<String> {
    let file_bytes = std::fs::read(path).ok()?;
    let hash_input = &file_bytes[..file_bytes.len().min(65536)];
    Some(blake3::hash(hash_input).to_hex().to_string())
}
