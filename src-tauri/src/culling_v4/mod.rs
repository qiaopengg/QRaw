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
    let culling_models = match models::get_or_init_culling_models_v4(&app_handle).await {
        Ok(m) => {
            let _ = app_handle.emit("culling-debug", format!(
                "[Models] OK: face={} yunet={} landmark={} expression={} nima_aes={} nima_tech={}",
                true,
                m.yunet_detector.is_some(),
                m.landmark_106.is_some(),
                m.expression_model.is_some(),
                m.nima_aesthetic.is_some(),
                m.nima_technical.is_some(),
            ));
            Some(m)
        }
        Err(e) => {
            let msg = format!("[Models] FAILED: {}", e);
            let _ = app_handle.emit("culling-debug", &msg);
            eprintln!("{}", msg);
            None
        }
    };

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
    let skip_portrait_stage = !settings.enable_auto_scene
        && matches!(
            settings.manual_profile,
            SceneType::Landscape | SceneType::Architecture
        );

    if skip_portrait_stage {
        let _ = app_handle.emit(
            "culling-debug",
            format!(
                "[Stage3] Skipped portrait analysis for manual scene={}",
                settings.manual_profile
            ),
        );
    } else {
        let _ = app_handle.emit("culling-debug", format!("[Stage3] Starting with {} assets, models={}", registry.assets.len(), culling_models.is_some()));
    }

    let portraits = if skip_portrait_stage {
        registry
            .assets
            .iter()
            .enumerate()
            .map(|(i, _)| PortraitVerdict {
                asset_index: i,
                has_faces: false,
                primary_face_area_ratio: 0.0,
                faces: vec![],
                composition_score: 0.5,
            })
            .collect()
    } else if let Some(ref models) = culling_models {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            stage3_portrait::stage_3_portrait(
                &registry, &verdicts, models, &settings, &app_handle,
            )
        }));
        match result {
            Ok(p) => {
                let _ = app_handle.emit("culling-debug", format!("[Stage3] Completed: {} portraits", p.len()));
                p
            }
            Err(e) => {
                let msg = format!("[Stage3] PANIC: {:?}", e);
                let _ = app_handle.emit("culling-debug", &msg);
                eprintln!("{}", msg);
                registry.assets.iter().enumerate().map(|(i, _)| {
                    PortraitVerdict { asset_index: i, has_faces: false, primary_face_area_ratio: 0.0, faces: vec![], composition_score: 0.5 }
                }).collect()
            }
        }
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
    // If user explicitly chose a manual scene, honor it even when auto-scene is enabled.
    let scene = if !matches!(settings.manual_profile, SceneType::Default) {
        settings.manual_profile.clone()
    } else if settings.enable_auto_scene {
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
    // Our NIMA ONNX uses NHWC format (TensorFlow export), not NCHW
    let nima_aesthetic_scores: Vec<Option<f64>> = if !settings.enable_nima_aesthetic {
        vec![None; registry.assets.len()]
    } else {
        if let Some(ref models) = culling_models {
            if let Some(ref nima_model) = models.nima_aesthetic {
                registry.assets.iter().enumerate().map(|(i, asset)| {
                    if verdicts[i].is_fail() { return None; }
                    score_nima_nhwc(&*asset.thumbnail, nima_model).ok()
                }).collect()
            } else {
                vec![None; registry.assets.len()]
            }
        } else {
            vec![None; registry.assets.len()]
        }
    };

    // ── Stage 4: Final Scoring ──
    let _ = app_handle.emit(
        "culling-progress",
        CullingProgressV4 { current: 0, total: 0, stage: "Computing final ratings...".into() },
    );

    let _ = app_handle.emit("culling-debug", format!("[Stage4] Starting scoring for {} assets, scene={}", registry.assets.len(), scene));
    let ratings = stage4_score::stage_4_score(
        &registry, &verdicts, &groups, &portraits,
        &scene, &nima_aesthetic_scores, &clip_quality_scores, &settings,
        &app_handle,
    );
    let _ = app_handle.emit("culling-debug", format!("[Stage4] Done: {} ratings", ratings.len()));

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

    // Emit statistics separately for the frontend overview
    let _ = app_handle.emit("culling-statistics", json!({
        "totalAnalyzed": registry.assets.len(),
        "ratingDistribution": rating_dist,
        "burstGroups": groups.len(),
        "duplicates": duplicates_count,
        "technicalFailures": technical_failures,
        "blinkDetected": blink_detected,
        "detectedScene": scene.to_string(),
        "elapsedMs": elapsed,
        "modelsLoaded": {
            "faceDetection": true,
            "landmark106": culling_models.as_ref().map(|m| m.landmark_106.is_some()).unwrap_or(false),
            "hsemotion": culling_models.as_ref().map(|m| m.expression_model.is_some()).unwrap_or(false),
            "nimaAesthetic": culling_models.as_ref().map(|m| m.nima_aesthetic.is_some()).unwrap_or(false),
            "nimaTechnical": culling_models.as_ref().map(|m| m.nima_technical.is_some()).unwrap_or(false),
            "clip": clip_models.is_some(),
        }
    }));

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

/// Score image using NIMA model (NHWC format from TensorFlow export)
fn score_nima_nhwc(
    image: &image::DynamicImage,
    model: &std::sync::Mutex<ort::session::Session>,
) -> Result<f64, String> {
    let size = 224u32;
    let rgb = image.to_rgb8();
    let resized = image::imageops::resize(&rgb, size, size, image::imageops::FilterType::Triangle);

    // NHWC format: (1, 224, 224, 3) — TensorFlow convention
    let mut arr = ndarray::Array4::<f32>::zeros((1, size as usize, size as usize, 3));
    let mean = [0.485f32, 0.456, 0.406];
    let std_dev = [0.229f32, 0.224, 0.225];
    for (x, y, p) in resized.enumerate_pixels() {
        arr[[0, y as usize, x as usize, 0]] = (p[0] as f32 / 255.0 - mean[0]) / std_dev[0];
        arr[[0, y as usize, x as usize, 1]] = (p[1] as f32 / 255.0 - mean[1]) / std_dev[1];
        arr[[0, y as usize, x as usize, 2]] = (p[2] as f32 / 255.0 - mean[2]) / std_dev[2];
    }

    let input = ort::value::Tensor::from_array(arr.into_dyn().as_standard_layout().into_owned())
        .map_err(|e| e.to_string())?;
    let output = {
        let mut sess = model.lock().unwrap();
        let outputs = sess.run(ort::inputs![input]).map_err(|e| e.to_string())?;
        outputs[0].try_extract_array::<f32>().map_err(|e| e.to_string())?.to_owned()
    };

    let flat = output.into_raw_vec_and_offset().0;
    if flat.len() < 10 { return Ok(5.0); }

    // Compute weighted mean score (1-10)
    let probs = if flat.iter().all(|v| (0.0..=1.0).contains(v)) {
        flat[..10].to_vec()
    } else {
        // Apply softmax if not already probabilities
        let max_v = flat[..10].iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = flat[..10].iter().map(|&x| (x - max_v).exp()).collect();
        let sum: f32 = exps.iter().sum();
        exps.into_iter().map(|e| e / sum.max(1e-8)).collect()
    };

    let mean_score: f64 = probs.iter().enumerate()
        .map(|(i, &p)| (i as f64 + 1.0) * p as f64)
        .sum();

    Ok(mean_score.clamp(1.0, 10.0))
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
