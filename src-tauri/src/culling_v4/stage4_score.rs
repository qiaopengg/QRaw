use super::types::*;

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
        if let Some(pv) = portrait {
            // Eye closed
            for face in &pv.faces {
                if face.area_ratio > 0.03 && !face.is_extreme_profile && face.is_eye_closed {
                    hard_penalty += 2;
                    reasons.push("eyesClosed".into());
                    break;
                }
            }
            // Mouth wide open
            for face in &pv.faces {
                if face.area_ratio > 0.05 && face.mouth_open_ratio > 0.5 {
                    hard_penalty += 1;
                    reasons.push("mouthWideOpen".into());
                    break;
                }
            }
            // Negative expression + physical indicator double verification
            for face in &pv.faces {
                if face.area_ratio > 0.03
                    && face.negative_emotion_prob > 0.6
                    && face.mouth_corner_down > 0.3
                {
                    hard_penalty += 1;
                    reasons.push("negativeExpression".into());
                    break;
                }
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
                let dr = *dynamic_range;
                let nima_t = nima_technical.map(|n| (n / 10.0).clamp(0.0, 1.0)).unwrap_or(blur);
                let scores = [blur, exp, dr, nima_t];
                let min_s = scores.iter().cloned().fold(f64::MAX, f64::min);
                let avg_s = scores.iter().sum::<f64>() / scores.len() as f64;
                min_s * 0.6 + avg_s * 0.4
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
                let smile = pv.faces.iter()
                    .filter(|f| f.area_ratio > 0.03)
                    .map(|f| f.smile_prob * 0.03)
                    .sum::<f64>()
                    .min(0.06);
                let comp = (pv.composition_score - 0.5) * 0.12;
                let all_open = pv.faces.iter()
                    .filter(|f| f.area_ratio > 0.03 && !f.is_extreme_profile)
                    .all(|f| !f.is_eye_closed);
                let eye_bonus = if all_open && pv.faces.len() >= 2 { 0.04 } else { 0.0 };
                (smile + comp + eye_bonus).clamp(-0.12, 0.12)
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
            // Full model: tech + portrait + aesthetic
            match scene {
                SceneType::CloseUpPortrait | SceneType::HalfBodyPortrait =>
                    tech_q * 0.65 + portrait_mod * 1.5 + aesthetic_mod,
                SceneType::Landscape | SceneType::Architecture =>
                    tech_q * 0.75 + aesthetic_mod * 2.0,
                SceneType::GroupPhoto | SceneType::Wedding =>
                    tech_q * 0.55 + portrait_mod * 2.0 + aesthetic_mod,
                SceneType::Action =>
                    tech_q * 0.70 + portrait_mod + aesthetic_mod,
                _ =>
                    tech_q * 0.65 + portrait_mod * 1.5 + aesthetic_mod,
            }
        } else {
            // Degraded mode: no NIMA/CLIP, tech_q must carry the score
            let base = tech_q * 0.90 + portrait_mod * 1.0;
            // Composition bonus (from rule engine, always available)
            let comp_bonus = (comp_score - 0.5) * 0.10;
            (base + comp_bonus).clamp(0.0, 1.0)
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
