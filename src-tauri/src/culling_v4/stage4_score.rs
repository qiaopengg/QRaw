use super::types::*;
use tauri::Emitter;

/// Normalize sharpness to 0.0~1.0 using exponential mapping
fn normalize_sharpness(raw: f64, threshold: f64) -> f64 {
    let k = (2.0f64.ln()) / threshold.max(1.0);
    (1.0 - (-k * raw).exp()).clamp(0.0, 1.0)
}

/// Compute aesthetic score from up to 3 signals (NIMA + CLIP + composition)
fn compute_aesthetic_mod(
    nima_aesthetic: Option<f64>,
    clip_quality: Option<f64>,
    composition_score: f64,
) -> f64 {
    let mut signals: Vec<f64> = vec![];
    if let Some(nima) = nima_aesthetic {
        signals.push((nima - 5.0) / 5.0); // Normalize 1-10 → -1~+1
    }
    if let Some(cq) = clip_quality {
        signals.push((cq - 0.5) * 2.0); // Normalize 0-1 → -1~+1
    }
    signals.push((composition_score - 0.5) * 2.0); // Always available

    if signals.is_empty() { return 0.0; }
    signals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = signals[signals.len() / 2];
    (median * 0.10).clamp(-0.10, 0.10)
}

fn expression_thresholds(strictness: &str) -> (f64, f64, f64) {
    match strictness {
        // Keep more photos unless expression is clearly problematic.
        "conservative" => (0.16, 0.72, 0.86),
        // Flag more expression issues.
        "aggressive" => (0.24, 0.45, 0.68),
        // Default balanced profile.
        _ => (0.20, 0.58, 0.78),
    }
}

/// Stage 4: Final scoring with decision tree
pub fn stage_4_score(
    registry: &AssetRegistry,
    verdicts: &[TechnicalVerdict],
    groups: &[BurstGroup],
    portraits: &[PortraitVerdict],
    scene: &SceneType,
    nima_aesthetic_scores: &[Option<f64>],
    clip_quality_scores: &[Option<f64>],
    settings: &CullingSettingsV4,
    app_handle: &tauri::AppHandle,
) -> Vec<FinalRating> {
    // Build group lookup: asset_index → (group, is_cover)
    let mut group_lookup: std::collections::HashMap<usize, (&BurstGroup, bool)> =
        std::collections::HashMap::new();
    for group in groups {
        for member in &group.members {
            group_lookup.insert(member.asset_index, (group, member.is_cover));
        }
    }

    // Compute cover quality for each group
    let mut cover_qualities: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();

    registry.assets.iter().enumerate().map(|(i, asset)| {
        let tech = &verdicts[i];
        let portrait = portraits.get(i);
        let group_info = group_lookup.get(&i).copied();
        let nima_aes = nima_aesthetic_scores.get(i).copied().flatten();
        let clip_q = clip_quality_scores.get(i).copied().flatten();
        let comp_score = portrait.map(|p| p.composition_score).unwrap_or(0.5);
        let (smile_low_thresh, negative_expr_thresh, severe_expr_thresh) =
            expression_thresholds(&settings.strictness);

        let mut reasons: Vec<String> = vec![];
        let mut hard_penalty = 0i32;

        // ═══ Layer 1: Technical veto ═══
        if let TechnicalVerdict::Fail { reason } = tech {
            return FinalRating {
                path: asset.path.clone(),
                stars: 1,
                reasons: vec![reason.to_tag()],
                quality_score: 0.0,
                breakdown: ScoreBreakdown {
                    scene_type: scene.to_string(),
                    ..Default::default()
                },
            };
        }

        // ═══ Layer 2: Blink hard constraint ═══
        // Safety check: if >80% of all faces across all photos are "closed",
        // the landmark indices are likely wrong. Skip blink penalty entirely.
        let total_faces_with_eyes: usize = portraits.iter()
            .flat_map(|p| p.faces.iter())
            .filter(|f| f.area_ratio > 0.03 && !f.is_extreme_profile)
            .count();
        let closed_faces: usize = portraits.iter()
            .flat_map(|p| p.faces.iter())
            .filter(|f| f.area_ratio > 0.03 && !f.is_extreme_profile && f.is_eye_closed)
            .count();
        let blink_detection_reliable = total_faces_with_eyes < 3
            || (closed_faces as f64 / total_faces_with_eyes.max(1) as f64) < 0.8;

        if let Some(pv) = portrait {
            // Eye closed (only if detection seems reliable)
            if blink_detection_reliable {
                for face in &pv.faces {
                    if face.area_ratio > 0.03 && !face.is_extreme_profile && face.is_eye_closed {
                        hard_penalty += 2;
                        reasons.push("eyesClosed".into());
                        break;
                    }
                }
            }
            // Mouth wide open — DISABLED until landmark indices verified
            // TODO: Re-enable after Phase 0 landmark verification
            // Keeping as soft signal in portrait_mod instead of hard_penalty
            /*
            for face in &pv.faces {
                if face.area_ratio > 0.05 && face.mouth_open_ratio > 0.5 {
                    hard_penalty += 1;
                    reasons.push("mouthWideOpen".into());
                    break;
                }
            }
            */
            // Expression penalties based on expression model outputs.
            // We do not rely on mouth-corner landmarks here.
            let mut has_negative_expr = false;
            let mut has_unnatural_expr = false;
            for face in &pv.faces {
                if face.area_ratio <= 0.03 {
                    continue;
                }

                let smile_low = face.smile_prob < smile_low_thresh;
                let neg_prob = face.negative_emotion_prob;
                let label = face.emotion_label.as_str();
                let negative_label = matches!(
                    label,
                    "anger" | "disgust" | "fear" | "sadness" | "contempt"
                );
                let unnatural_label = matches!(label, "surprise");

                if (neg_prob >= negative_expr_thresh && smile_low)
                    || (negative_label && neg_prob >= negative_expr_thresh * 0.65 && smile_low)
                {
                    has_negative_expr = true;
                }
                if (neg_prob >= severe_expr_thresh && smile_low)
                    || (unnatural_label && smile_low)
                {
                    has_unnatural_expr = true;
                }
            }
            if has_negative_expr {
                hard_penalty += 1;
                reasons.push("negativeExpression".into());
            }
            if has_unnatural_expr {
                hard_penalty += 1;
                reasons.push("unnaturalExpression".into());
            }
            // Face cropped
            if pv.faces.iter().any(|f| f.is_edge_cropped && f.area_ratio > 0.05) {
                hard_penalty += 1;
                reasons.push("faceCropped".into());
            }
        }

        // ═══ Layer 3: Technical quality (shortboard effect) ═══
        let tech_q = match tech {
            TechnicalVerdict::Pass {
                sharpness,
                subject_sharpness,
                exposure_health,
                dynamic_range,
                nima_technical,
            } => {
                let blur = normalize_sharpness(*subject_sharpness, settings.blur_threshold);
                let exp = *exposure_health;
                // dynamic_range removed from shortboard — normal portraits have narrow DR
                // nima_technical as bonus if available
                let nima_bonus = nima_technical.map(|n| ((n / 10.0) - 0.5) * 0.1).unwrap_or(0.0);
                let min_s = blur.min(exp);
                let avg_s = (blur + exp) / 2.0;
                (min_s * 0.5 + avg_s * 0.5 + nima_bonus).clamp(0.0, 1.0)
            }
            TechnicalVerdict::Marginal { sharpness, exposure_health, reason } => {
                reasons.push(reason.to_tag());
                (normalize_sharpness(*sharpness, settings.blur_threshold).min(*exposure_health)) * 0.5
            }
            _ => 0.0,
        };

        // ═══ Layer 4: Portrait modifier ═══
        let portrait_mod = if let Some(pv) = portrait {
            if !pv.has_faces {
                0.0
            } else {
                let neg_weight = match settings.strictness.as_str() {
                    "conservative" => 0.06,
                    "aggressive" => 0.12,
                    _ => 0.09,
                };
                let smile = pv.faces.iter()
                    .filter(|f| f.area_ratio > 0.03)
                    .map(|f| f.smile_prob * 0.03)
                    .sum::<f64>()
                    .min(0.06);
                let neg_penalty = pv.faces.iter()
                    .filter(|f| f.area_ratio > 0.03)
                    .map(|f| f.negative_emotion_prob * neg_weight)
                    .sum::<f64>()
                    .min(0.12);
                let comp = (pv.composition_score - 0.5) * 0.12;
                let all_open = pv.faces.iter()
                    .filter(|f| f.area_ratio > 0.03 && !f.is_extreme_profile)
                    .all(|f| !f.is_eye_closed);
                let eye_bonus = if all_open && pv.faces.len() >= 2 { 0.04 } else { 0.0 };
                (smile + comp + eye_bonus - neg_penalty).clamp(-0.12, 0.12)
            }
        } else {
            0.0
        };

        // ═══ Layer 5: Aesthetic (3-signal arbitration) ═══
        let aesthetic_mod = compute_aesthetic_mod(nima_aes, clip_q, comp_score);

        // ═══ Layer 6: Scene-adaptive weighting ═══
        // When NIMA/CLIP are unavailable, boost tech_q weight to compensate
        let has_aesthetic = nima_aes.is_some() || clip_q.is_some();
        let final_q = if has_aesthetic {
            match scene {
                SceneType::CloseUpPortrait | SceneType::HalfBodyPortrait =>
                    tech_q * 0.80 + portrait_mod * 2.0 + aesthetic_mod * 1.5,
                SceneType::Landscape | SceneType::Architecture =>
                    tech_q * 0.85 + aesthetic_mod * 2.5,
                SceneType::GroupPhoto | SceneType::Wedding =>
                    tech_q * 0.75 + portrait_mod * 2.5 + aesthetic_mod * 1.5,
                SceneType::Action =>
                    tech_q * 0.85 + portrait_mod * 1.5 + aesthetic_mod,
                _ =>
                    tech_q * 0.80 + portrait_mod * 2.0 + aesthetic_mod * 1.5,
            }
        } else {
            let base = tech_q * 0.95;
            let comp_bonus = (comp_score - 0.5) * 0.05;
            (base + portrait_mod + comp_bonus).clamp(0.0, 1.0)
        }.clamp(0.0, 1.0);

        // ═══ Layer 7: Burst duplicate demotion ═══
        let mut adjusted = final_q;
        if let Some((group, is_cover)) = group_info {
            if !is_cover {
                reasons.push("burstDuplicate".into());
                // Get cover quality (compute or lookup)
                let cover_q = cover_qualities.entry(group.group_id.clone()).or_insert_with(|| {
                    let cover_tech = &verdicts[group.cover_index];
                    cover_tech.subject_sharpness_or(0.5)
                });
                adjusted = adjusted.min(*cover_q - 0.15);
            }
        }

        // ═══ Star mapping ═══
        let base_stars = if adjusted > 0.80 { 5 }
            else if adjusted > 0.62 { 4 }
            else if adjusted > 0.42 { 3 }
            else if adjusted > 0.22 { 2 }
            else { 1 };

        let stars = (base_stars as i32 - hard_penalty).clamp(1, 5) as u8;

        // Debug logging
        let debug_msg = format!(
            "[Score] {} → tech_q={:.3} portrait={:.3} aesthetic={:.3} final={:.3} penalty={} → {}★ reasons={:?}",
            asset.path.split('/').last().unwrap_or(&asset.path),
            tech_q, portrait_mod, aesthetic_mod, adjusted, hard_penalty, stars, reasons
        );
        let _ = app_handle.emit("culling-debug", &debug_msg);

        // Add positive reasons
        if let Some(pv) = portrait {
            if pv.faces.iter().any(|f| f.smile_prob > 0.6) {
                reasons.push("smileGood".into());
            }
        }

        FinalRating {
            path: asset.path.clone(),
            stars,
            reasons,
            quality_score: adjusted,
            breakdown: ScoreBreakdown {
                sharpness: tech.subject_sharpness_or(0.0),
                subject_sharpness: tech.subject_sharpness_or(0.0),
                exposure: tech.exposure_health_or(0.0),
                dynamic_range: match tech {
                    TechnicalVerdict::Pass { dynamic_range, .. } => *dynamic_range,
                    _ => 0.0,
                },
                nima_technical: match tech {
                    TechnicalVerdict::Pass { nima_technical, .. } => *nima_technical,
                    _ => None,
                },
                nima_aesthetic: nima_aes,
                clip_quality: clip_q,
                face_blink: portrait.map(|p| p.faces.iter().any(|f| f.is_eye_closed)),
                face_expression: portrait.map(|p| {
                    let faces_with_expr: Vec<_> = p.faces.iter().filter(|f| f.area_ratio > 0.03).collect();
                    if faces_with_expr.is_empty() { 0.0 }
                    else { faces_with_expr.iter().map(|f| f.smile_prob).sum::<f64>() / faces_with_expr.len() as f64 }
                }),
                face_composition: portrait.map(|p| p.composition_score),
                composition: comp_score,
                scene_type: scene.to_string(),
                group_id: group_info.map(|(g, _)| g.group_id.clone()),
                is_cover: group_info.map(|(_, c)| c).unwrap_or(false),
            },
        }
    }).collect()
}
