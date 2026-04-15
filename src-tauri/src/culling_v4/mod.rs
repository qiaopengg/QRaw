pub mod cache;
pub mod clip_quality;
pub mod composition;
pub mod landmarks;
pub mod models;
pub mod scene_detect;
pub mod stage0_discover;
pub mod stage1_technical;
pub mod stage2_dedup;
pub mod stage3_portrait;
pub mod stage4_score;
pub mod types;

use std::sync::Arc;
use std::time::Instant;

use serde_json::json;
use tauri::{AppHandle, Emitter, State};

use crate::ai_processing::{get_or_init_clip_models, ClipModels};
use crate::file_management::parse_virtual_path;
use crate::image_processing::ImageMetadata;

use types::*;

/// Main entry point: cull_images_v4 Tauri command
#[tauri::command]
pub async fn cull_images_v4(
    paths: Vec<String>,
    settings: CullingSettingsV4,
    state: State<'_, crate::AppState>,
    app_handle: AppHandle,
) -> Result<CullingResultV4, String> {
    let start_time = Instant::now();

    if paths.is_empty() {
        return Ok(CullingResultV4 {
            statistics: CullingStatisticsV4 {
                total_analyzed: 0,
                total_primary: 0,
                rating_distribution: [0; 6],
                burst_groups_count: 0,
                duplicates_count: 0,
                technical_failures: 0,
                blink_detected: 0,
                detected_scene: "default".into(),
                elapsed_ms: 0,
            },
            ratings: vec![],
            burst_groups: vec![],
        });
    }

    // ── Stage 0: Asset Discovery ──
    let registry = stage0_discover::stage_0_discover(&paths, &app_handle)
        .map_err(|e| format!("Stage 0 failed: {}", e))?;

    // ── Initialize models ──
    let culling_models = models::get_or_init_culling_models_v4(
        &app_handle,
    ).await.ok();

    // Try to get CLIP models (with timeout, don't block if unavailable)
    let clip_models: Option<Arc<ClipModels>> = match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        get_or_init_clip_models(&app_handle, &state.ai_state, &state.ai_init_lock),
    ).await {
        Ok(Ok(m)) => Some(m),
        _ => None,
    };

    // ── Stage 1: Technical Elimination ──
    // Depth Anything: try to get from existing AiState (don't trigger download)
    let depth_available = state.ai_state.lock().unwrap()
        .as_ref()
        .and_then(|s| s.models.as_ref())
        .is_some();

    // Depth Anything: currently None (TODO: integrate properly)
    // When available, pass a closure that runs depth inference
    let depth_fn_ref: Option<&(dyn Fn(&image::DynamicImage) -> Option<image::GrayImage> + Sync)> = None;

    let verdicts = stage1_technical::stage_1_technical(
        &registry,
        depth_fn_ref,
        &settings,
        &app_handle,
    );

    // ── Stage 2: Burst Deduplication ──
    let groups = stage2_dedup::stage_2_dedup(
        &registry, &verdicts, &settings, &app_handle,
    );

    // ── Stage 3: Portrait Assessment ──
    let portraits = if let Some(ref models) = culling_models {
        stage3_portrait::stage_3_portrait(
            &registry, &verdicts, models, &settings, &app_handle,
        )
    } else {
        // No models available: generate empty portrait verdicts
        registry.assets.iter().enumerate().map(|(i, _)| {
            PortraitVerdict {
                asset_index: i,
                has_faces: false,
                primary_face_area_ratio: 0.0,
                faces: vec![],
                composition_score: 0.5,
            }
        }).collect()
    };

    // ── Scene Detection ──
    let scene = if settings.enable_auto_scene {
        scene_detect::auto_detect_scene(&portraits)
    } else {
        settings.manual_profile.clone()
    };

    // ── CLIP Quality Scores (optional) ──
    let clip_quality_scores: Vec<Option<f64>> = if let Some(ref clip) = clip_models {
        let _ = app_handle.emit(
            "culling-progress",
            CullingProgressV4 { current: 0, total: 0, stage: "Computing quality scores...".into() },
        );
        registry.assets.iter().enumerate().map(|(i, asset)| {
            if verdicts[i].is_fail() { return None; }
            clip_quality::compute_clip_quality(&*asset.thumbnail, clip)
        }).collect()
    } else {
        vec![None; registry.assets.len()]
    };

    // ── NIMA Aesthetic Scores (optional) ──
    let nima_aesthetic_scores: Vec<Option<f64>> = if let Some(ref models) = culling_models {
        if let Some(ref nima_model) = models.nima_aesthetic {
            registry.assets.iter().enumerate().map(|(i, asset)| {
                if verdicts[i].is_fail() { return None; }
                crate::ai_processing::score_aesthetics_nima(&*asset.thumbnail, nima_model)
                    .ok()
                    .map(|s| s as f64)
            }).collect()
        } else {
            vec![None; registry.assets.len()]
        }
    } else {
        vec![None; registry.assets.len()]
    };

    // ── Stage 4: Final Scoring ──
    let _ = app_handle.emit(
        "culling-progress",
        CullingProgressV4 { current: 0, total: 0, stage: "Computing final ratings...".into() },
    );

    let ratings = stage4_score::stage_4_score(
        &registry, &verdicts, &groups, &portraits,
        &scene, &nima_aesthetic_scores, &clip_quality_scores, &settings,
    );

    // ── Persist results to .rrdata ──
    persist_ratings(&registry, &ratings, &app_handle).await;

    // ── Build statistics ──
    let mut rating_dist = [0usize; 6];
    for r in &ratings {
        if r.stars >= 1 && r.stars <= 5 {
            rating_dist[r.stars as usize] += 1;
        }
    }

    let duplicates_count: usize = groups.iter()
        .map(|g| g.members.len().saturating_sub(1))
        .sum();

    let technical_failures = verdicts.iter().filter(|v| v.is_fail()).count();
    let blink_detected = portraits.iter()
        .filter(|p| p.faces.iter().any(|f| f.is_eye_closed))
        .count();

    let elapsed = start_time.elapsed().as_millis() as u64;

    // ── Emit legacy-compatible CullingSuggestions for frontend ──
    let legacy_suggestions = build_legacy_suggestions(&registry, &ratings, &groups, &portraits);
    let _ = app_handle.emit("culling-complete", &legacy_suggestions);

    Ok(CullingResultV4 {
        statistics: CullingStatisticsV4 {
            total_analyzed: registry.assets.len(),
            total_primary: registry.primary_count(),
            rating_distribution: rating_dist,
            burst_groups_count: groups.len(),
            duplicates_count,
            technical_failures,
            blink_detected,
            detected_scene: scene.to_string(),
            elapsed_ms: elapsed,
        },
        ratings,
        burst_groups: groups,
    })
}

/// Persist ratings to .rrdata sidecar files
async fn persist_ratings(
    registry: &AssetRegistry,
    ratings: &[FinalRating],
    app_handle: &AppHandle,
) {
    for (i, rating) in ratings.iter().enumerate() {
        let path = &registry.assets[i].path;
        let (_, sidecar_path) = parse_virtual_path(path);

        let mut metadata: ImageMetadata = if sidecar_path.exists() {
            std::fs::read_to_string(&sidecar_path)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
                .unwrap_or_default()
        } else {
            ImageMetadata::default()
        };

        let mut adjustments = metadata.adjustments;
        if adjustments.is_null() {
            adjustments = json!({});
        }
        if let Some(map) = adjustments.as_object_mut() {
            map.insert("rating".to_string(), json!(rating.stars));
            map.insert("aiCulling".to_string(), json!({
                "version": 4,
                "stars": rating.stars,
                "qualityScore": rating.quality_score,
                "reasons": rating.reasons,
                "breakdown": rating.breakdown,
                "groupId": rating.breakdown.group_id,
                "isCover": rating.breakdown.is_cover,
            }));
        }
        metadata.adjustments = adjustments;
        metadata.rating = rating.stars;

        if let Ok(json_str) = serde_json::to_string_pretty(&metadata) {
            let _ = std::fs::write(&sidecar_path, json_str);
        }

        // Propagate to JPEG pair if exists
        if let Some(jpeg_path) = registry.raw_jpeg_pairs.get(path) {
            let (_, jpeg_sidecar) = parse_virtual_path(jpeg_path);
            let mut jpeg_meta: ImageMetadata = if jpeg_sidecar.exists() {
                std::fs::read_to_string(&jpeg_sidecar)
                    .ok()
                    .and_then(|c| serde_json::from_str(&c).ok())
                    .unwrap_or_default()
            } else {
                ImageMetadata::default()
            };
            jpeg_meta.rating = rating.stars;
            let mut adj = jpeg_meta.adjustments;
            if adj.is_null() { adj = json!({}); }
            if let Some(map) = adj.as_object_mut() {
                map.insert("rating".to_string(), json!(rating.stars));
            }
            jpeg_meta.adjustments = adj;
            if let Ok(json_str) = serde_json::to_string_pretty(&jpeg_meta) {
                let _ = std::fs::write(&jpeg_sidecar, json_str);
            }
        }
    }

    // Batch update ratings via existing mechanism
    let mut by_rating: std::collections::HashMap<u8, Vec<String>> = std::collections::HashMap::new();
    for rating in ratings {
        by_rating.entry(rating.stars).or_default().push(rating.path.clone());
    }
    for (star, paths) in by_rating {
        let _ = crate::file_management::apply_adjustments_to_paths(
            paths,
            json!({ "rating": star }),
            app_handle.clone(),
        ).await;
    }
}

/// Build legacy CullingSuggestions from V4 results for frontend compatibility
fn build_legacy_suggestions(
    _registry: &AssetRegistry,
    ratings: &[FinalRating],
    groups: &[BurstGroup],
    _portraits: &[PortraitVerdict],
) -> LegacyCullingSuggestions {
    let to_legacy = |i: usize, r: &FinalRating| -> LegacyImageAnalysisResult {
        LegacyImageAnalysisResult {
            path: r.path.clone(),
            quality_score: r.quality_score,
            calibrated_score: r.quality_score,
            sharpness_metric: r.breakdown.sharpness,
            center_focus_metric: r.breakdown.subject_sharpness,
            exposure_metric: r.breakdown.exposure,
            face_score: r.breakdown.face_expression,
            aesthetic_score: r.breakdown.nima_aesthetic,
            width: 0,
            height: 0,
            suggested_rating: r.stars,
            reasons: r.reasons.clone(),
            score_breakdown: {
                let mut map = std::collections::HashMap::new();
                map.insert("sharpness".into(), r.breakdown.sharpness);
                map.insert("exposure".into(), r.breakdown.exposure);
                map.insert("composition".into(), r.breakdown.composition);
                if let Some(v) = r.breakdown.nima_aesthetic {
                    map.insert("aesthetic".into(), v);
                }
                map
            },
            face_detector_type: None,
            group_id: r.breakdown.group_id.clone(),
            is_cover: r.breakdown.is_cover,
        }
    };

    let similar_groups: Vec<LegacyCullGroup> = groups.iter().map(|g| {
        let cover_rating = &ratings[g.cover_index];
        let representative = to_legacy(g.cover_index, cover_rating);
        let duplicates: Vec<LegacyImageAnalysisResult> = g.members.iter()
            .filter(|m| !m.is_cover)
            .map(|m| to_legacy(m.asset_index, &ratings[m.asset_index]))
            .collect();
        LegacyCullGroup { representative, duplicates }
    }).collect();

    let blurry_images: Vec<LegacyImageAnalysisResult> = ratings.iter().enumerate()
        .filter(|(_, r)| r.reasons.iter().any(|reason| reason.contains("Blur") || reason.contains("blur")))
        .map(|(i, r)| to_legacy(i, r))
        .collect();

    let bad_expressions: Vec<LegacyImageAnalysisResult> = ratings.iter().enumerate()
        .filter(|(_, r)| r.reasons.iter().any(|reason|
            reason == "eyesClosed" || reason == "mouthWideOpen" ||
            reason == "negativeExpression" || reason == "unnaturalExpression"
        ))
        .map(|(i, r)| to_legacy(i, r))
        .collect();

    LegacyCullingSuggestions {
        similar_groups,
        blurry_images,
        bad_expressions,
        failed_paths: vec![],
    }
}
