use super::types::*;

/// Auto-detect scene type from portrait analysis results
/// Uses face statistics first (fast), CLIP zero-shot as fallback
pub fn auto_detect_scene(portraits: &[PortraitVerdict]) -> SceneType {
    let total = portraits.len().max(1) as f64;

    let large_face = portraits.iter()
        .filter(|p| p.primary_face_area_ratio > 0.08)
        .count() as f64;
    let multi_face = portraits.iter()
        .filter(|p| p.faces.len() >= 3)
        .count() as f64;
    let no_face = portraits.iter()
        .filter(|p| !p.has_faces)
        .count() as f64;

    // 80%+ photos have large face → portrait
    if large_face / total > 0.8 {
        return SceneType::CloseUpPortrait;
    }
    // 60%+ photos have 3+ faces → group photo
    if multi_face / total > 0.6 {
        return SceneType::GroupPhoto;
    }
    // 70%+ photos have no face → landscape
    if no_face / total > 0.7 {
        return SceneType::Landscape;
    }

    // Mixed content
    SceneType::Default
}
