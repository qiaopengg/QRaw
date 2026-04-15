use image_hasher::{HashAlg, HasherConfig};
use tauri::{AppHandle, Emitter};

use super::types::*;

/// Stage 2: Burst deduplication using perceptual hash + time window
pub fn stage_2_dedup(
    registry: &AssetRegistry,
    verdicts: &[TechnicalVerdict],
    settings: &CullingSettingsV4,
    app_handle: &AppHandle,
) -> Vec<BurstGroup> {
    let _ = app_handle.emit(
        "culling-progress",
        CullingProgressV4 { current: 0, total: 0, stage: "Detecting duplicates...".into() },
    );

    // Only process primary, non-failed assets
    let mut candidates: Vec<usize> = registry.assets.iter().enumerate()
        .filter(|(i, a)| a.is_primary && !verdicts[*i].is_fail())
        .map(|(i, _)| i)
        .collect();

    // Sort by capture time
    candidates.sort_by_key(|&i| registry.assets[i].capture_time);

    // Step 1: Split by time gap (3 seconds)
    let time_windows = split_by_time_gap(&candidates, registry, 3000);

    // Step 2: Perceptual hash clustering within each window
    let hasher = HasherConfig::new()
        .hash_alg(HashAlg::DoubleGradient)
        .hash_size(16, 16)
        .to_hasher();

    let mut groups = Vec::new();

    for window in &time_windows {
        if window.len() <= 1 { continue; }

        let hashes: Vec<(usize, image_hasher::ImageHash)> = window.iter()
            .map(|&i| (i, hasher.hash_image(&*registry.assets[i].thumbnail)))
            .collect();

        let clusters = cluster_by_hash(&hashes, settings.similarity_threshold);

        for cluster in clusters {
            if cluster.len() <= 1 { continue; }

            // Select cover by technical quality priority
            let cover = select_cover(&cluster, verdicts);

            groups.push(BurstGroup {
                group_id: format!("g{}", groups.len() + 1),
                cover_index: cover,
                members: cluster.iter().map(|&idx| BurstMember {
                    asset_index: idx,
                    is_cover: idx == cover,
                }).collect(),
            });
        }
    }

    groups
}

fn split_by_time_gap(indices: &[usize], registry: &AssetRegistry, gap_ms: i64) -> Vec<Vec<usize>> {
    let mut windows: Vec<Vec<usize>> = vec![];
    let mut current: Vec<usize> = vec![];

    for &idx in indices {
        if let Some(&last) = current.last() {
            let dt = registry.assets[idx].capture_time - registry.assets[last].capture_time;
            if dt > gap_ms {
                if current.len() > 1 {
                    windows.push(current.clone());
                }
                current.clear();
            }
        }
        current.push(idx);
    }
    if current.len() > 1 {
        windows.push(current);
    }
    windows
}

fn cluster_by_hash(hashes: &[(usize, image_hasher::ImageHash)], threshold: u32) -> Vec<Vec<usize>> {
    let mut visited = vec![false; hashes.len()];
    let mut clusters: Vec<Vec<usize>> = vec![];

    for i in 0..hashes.len() {
        if visited[i] { continue; }
        visited[i] = true;
        let mut cluster = vec![hashes[i].0];

        for j in (i + 1)..hashes.len() {
            if visited[j] { continue; }
            let dist = hashes[i].1.dist(&hashes[j].1);
            if dist <= threshold {
                visited[j] = true;
                cluster.push(hashes[j].0);
            }
        }
        clusters.push(cluster);
    }
    clusters
}

fn select_cover(cluster: &[usize], verdicts: &[TechnicalVerdict]) -> usize {
    *cluster.iter().max_by(|&&a, &&b| {
        let sa = verdicts[a].subject_sharpness_or(0.0);
        let sb = verdicts[b].subject_sharpness_or(0.0);
        sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let ea = verdicts[a].exposure_health_or(0.0);
                let eb = verdicts[b].exposure_health_or(0.0);
                ea.partial_cmp(&eb).unwrap_or(std::cmp::Ordering::Equal)
            })
    }).unwrap_or(&cluster[0])
}
