use std::collections::HashSet;
use std::time::{Duration, Instant};

use super::api::{FailureItem, InventorySummary, KeyPersonSelection};
use super::domain::{ColorLabel, ConfirmedResult, ResultSource, TaskState};
use super::infrastructure::{ApplyFailureReason, Catalog, CatalogAssetStatus, CatalogSkipReason};
use super::scoring::{MODEL_VERSION, POLICY_VERSION};

pub(crate) fn inventory_summary(catalog: &Catalog) -> InventorySummary {
    let eligible_assets = catalog
        .assets
        .iter()
        .filter(|asset| asset.status == CatalogAssetStatus::Eligible)
        .count();
    let protected_assets = catalog.assets.len() - eligible_assets;
    let folder_count = catalog
        .assets
        .iter()
        .filter_map(|asset| asset.primary_path.parent())
        .collect::<HashSet<_>>()
        .len();
    InventorySummary {
        total_assets: catalog.assets.len() + catalog.skipped.len() + catalog.failures.len(),
        eligible_assets,
        protected_assets,
        skipped_assets: catalog.skipped.len(),
        failed_assets: catalog.failures.len(),
        folder_count,
    }
}

pub(crate) fn catalog_failures(catalog: &Catalog) -> Vec<FailureItem> {
    let mut failures = Vec::new();
    for asset in catalog
        .assets
        .iter()
        .filter(|asset| asset.status == CatalogAssetStatus::Protected)
    {
        failures.push(FailureItem {
            path: asset.display_path.to_string_lossy().to_string(),
            member_paths: asset
                .member_paths
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect(),
            stage: "scan".to_string(),
            code: "manual_protected".to_string(),
            detail: "Existing user decision is protected and the whole asset was skipped"
                .to_string(),
            retryable: false,
        });
    }
    for skipped in &catalog.skipped {
        failures.push(FailureItem {
            path: skipped.paths[0].to_string_lossy().to_string(),
            member_paths: skipped
                .paths
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect(),
            stage: "scan".to_string(),
            code: match skipped.reason {
                CatalogSkipReason::ExcludedFormat => "excluded_format",
                CatalogSkipReason::AmbiguousPair => "ambiguous_pair",
            }
            .to_string(),
            detail: match skipped.reason {
                CatalogSkipReason::ExcludedFormat => {
                    "GIF and TIFF/TIF are not supported".to_string()
                }
                CatalogSkipReason::AmbiguousPair => {
                    "RAW/JPEG members are ambiguous, so the asset was not guessed".to_string()
                }
            },
            retryable: false,
        });
    }
    failures.extend(catalog.failures.iter().map(|failure| FailureItem {
        path: failure.path.to_string_lossy().to_string(),
        member_paths: vec![failure.path.to_string_lossy().to_string()],
        stage: "scan".to_string(),
        code: "scan_failed".to_string(),
        detail: failure.reason.clone(),
        retryable: false,
    }));
    failures
}

/// Upper bound on reference photos per key person. Several references make the
/// identity template robust to head angle and lighting, but the useful number is
/// small and each one costs an extra embedding pass. This bound is a conservative
/// engineering guard, not a frozen product limit; the final value belongs to the
/// real-photo calibration set (`DATA-01`).
pub(crate) const MAX_REFERENCES_PER_KEY_PERSON: usize = 5;

/// Checks the identity numbering and per-identity reference budget.
///
/// Identities are numbered 1..=N with no gaps, and each may carry several
/// reference photos, so this validates the set of distinct identities rather
/// than the raw selection count.
fn valid_identity_structure(selections: &[KeyPersonSelection]) -> bool {
    let mut identities = selections
        .iter()
        .map(|selection| selection.priority)
        .collect::<Vec<_>>();
    identities.sort_unstable();
    identities.dedup();
    if identities != (1..=identities.len()).collect::<Vec<_>>() {
        return false;
    }
    identities.iter().all(|identity| {
        let references = selections
            .iter()
            .filter(|selection| selection.priority == *identity)
            .count();
        (1..=MAX_REFERENCES_PER_KEY_PERSON).contains(&references)
    })
}

pub(crate) fn valid_key_people(selections: &[KeyPersonSelection], catalog: &Catalog) -> bool {
    if !valid_identity_structure(selections) {
        return false;
    }

    selections.iter().enumerate().all(|(index, selection)| {
        valid_normalized_bbox(selection.bbox)
            && catalog.assets.iter().any(|asset| {
                asset
                    .member_paths
                    .iter()
                    .any(|path| path == std::path::Path::new(&selection.sample_path))
            })
            && !selections[..index].iter().any(|earlier| {
                earlier.sample_path == selection.sample_path && earlier.bbox == selection.bbox
            })
    })
}

fn valid_normalized_bbox([x, y, width, height]: [f32; 4]) -> bool {
    [x, y, width, height].iter().all(|value| value.is_finite())
        && x >= 0.0
        && y >= 0.0
        && width > 0.0
        && height > 0.0
        && x + width <= 1.0
        && y + height <= 1.0
}

pub(crate) fn confirmed_result(
    result: &super::api::ReviewResult,
    confirmed_at: &str,
) -> Result<ConfirmedResult, String> {
    let source = if result.source == "manual" {
        ResultSource::Manual
    } else {
        ResultSource::Ai
    };
    Ok(ConfirmedResult {
        result_id: result.result_id.clone(),
        source,
        rating: result.rating,
        color_label: parse_color(result.color_label.as_deref())?,
        reason_codes: if source == ResultSource::Ai {
            result.reason_codes.clone()
        } else {
            Vec::new()
        },
        confidence: result.confidence,
        mode: if source == ResultSource::Ai {
            result.mode.clone()
        } else {
            String::new()
        },
        model_version: if source == ResultSource::Ai {
            MODEL_VERSION.to_string()
        } else {
            String::new()
        },
        policy_version: if source == ResultSource::Ai {
            POLICY_VERSION.to_string()
        } else {
            String::new()
        },
        confirmed_at: confirmed_at.to_string(),
    })
}

fn parse_color(value: Option<&str>) -> Result<Option<ColorLabel>, String> {
    match value {
        None => Ok(None),
        Some("green") => Ok(Some(ColorLabel::Green)),
        Some("yellow") => Ok(Some(ColorLabel::Yellow)),
        Some("red") => Ok(Some(ColorLabel::Red)),
        Some(other) => Err(format!("Unknown color label: {other}")),
    }
}

pub(crate) fn valid_color(value: Option<&str>) -> bool {
    matches!(value, None | Some("green" | "yellow" | "red"))
}

pub(crate) fn valid_mode(mode: &str) -> bool {
    matches!(
        mode,
        "auto" | "landscape" | "portrait" | "environment" | "group"
    )
}

pub(crate) fn mode_supports_key_people(mode: &str) -> bool {
    matches!(mode, "portrait" | "group")
}

pub(crate) fn state_name(state: TaskState) -> &'static str {
    match state {
        TaskState::Idle => "idle",
        TaskState::Preflighting => "preflighting",
        TaskState::Configuring => "configuring",
        TaskState::Indexing => "indexing",
        TaskState::Rendering => "rendering",
        TaskState::Analyzing => "analyzing",
        TaskState::Organizing => "organizing",
        TaskState::ReadyForReview => "readyForReview",
        TaskState::Confirming => "confirming",
        TaskState::Completed => "completed",
        TaskState::Cancelling => "cancelling",
        TaskState::Abandoning => "abandoning",
        TaskState::Failed => "failed",
        TaskState::Unsupported => "unsupported",
    }
}

pub(crate) fn eta_seconds(
    started_at: Option<Instant>,
    completed: usize,
    total: usize,
) -> Option<u64> {
    let started_at = started_at?;
    if completed == 0 || completed >= total {
        return None;
    }
    let elapsed = started_at.elapsed().as_secs_f64();
    let remaining = elapsed / completed as f64 * (total - completed) as f64;
    Some(Duration::from_secs_f64(remaining.max(0.0)).as_secs())
}

pub(crate) fn apply_failure_code(reason: ApplyFailureReason) -> &'static str {
    match reason {
        ApplyFailureReason::AssetChanged => "asset_changed",
        ApplyFailureReason::BaselineConflict => "baseline_conflict",
        ApplyFailureReason::ManualProtection => "manual_protected",
        ApplyFailureReason::InvalidResult => "invalid_result",
        ApplyFailureReason::Io => "io_error",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        KeyPersonSelection, MAX_REFERENCES_PER_KEY_PERSON, mode_supports_key_people,
        valid_identity_structure, valid_mode, valid_normalized_bbox,
    };

    fn selection(priority: usize, sample: &str) -> KeyPersonSelection {
        KeyPersonSelection {
            sample_path: sample.to_string(),
            bbox: [0.1, 0.1, 0.2, 0.2],
            priority,
        }
    }

    #[test]
    fn one_identity_may_carry_several_reference_photos() {
        let selections = vec![
            selection(1, "a.jpg"),
            selection(1, "b.jpg"),
            selection(1, "c.jpg"),
            selection(2, "a.jpg"),
        ];

        assert!(valid_identity_structure(&selections));
    }

    #[test]
    fn identity_numbering_must_stay_contiguous_from_one() {
        assert!(valid_identity_structure(&[selection(1, "a.jpg")]));
        // Gap at 2 and a set that does not start at 1 are both rejected.
        assert!(!valid_identity_structure(&[
            selection(1, "a.jpg"),
            selection(3, "b.jpg")
        ]));
        assert!(!valid_identity_structure(&[selection(2, "a.jpg")]));
    }

    #[test]
    fn reference_photos_per_identity_stay_within_the_budget() {
        let within = (0..MAX_REFERENCES_PER_KEY_PERSON)
            .map(|index| selection(1, &format!("{index}.jpg")))
            .collect::<Vec<_>>();
        assert!(valid_identity_structure(&within));

        let mut over_budget = within.clone();
        over_budget.push(selection(1, "extra.jpg"));
        assert!(!valid_identity_structure(&over_budget));
    }

    #[test]
    fn an_empty_selection_is_structurally_valid() {
        assert!(valid_identity_structure(&[]));
    }

    #[test]
    fn key_person_boxes_must_be_finite_and_inside_the_photo() {
        assert!(valid_normalized_bbox([0.1, 0.2, 0.3, 0.4]));
        assert!(!valid_normalized_bbox([0.8, 0.2, 0.3, 0.4]));
        assert!(!valid_normalized_bbox([f32::NAN, 0.2, 0.3, 0.4]));
        assert!(!valid_normalized_bbox([0.1, 0.2, 0.0, 0.4]));
    }

    #[test]
    fn accepts_only_the_five_supported_modes() {
        for mode in ["auto", "landscape", "portrait", "environment", "group"] {
            assert!(valid_mode(mode));
        }
        for mode in [
            "documentary",
            "wildlife",
            "architecture",
            "product",
            "astro",
        ] {
            assert!(!valid_mode(mode));
        }
    }

    #[test]
    fn key_people_are_limited_to_people_focused_modes() {
        for mode in ["portrait", "group"] {
            assert!(mode_supports_key_people(mode));
        }
        for mode in ["auto", "landscape", "environment"] {
            assert!(!mode_supports_key_people(mode));
        }
    }
}
