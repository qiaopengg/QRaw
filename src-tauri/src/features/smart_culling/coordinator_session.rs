use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use super::api::{
    DetectedFaceDto, DevicePreflight, FailureItem, InventorySummary, ReviewResult,
    SmartCullingSnapshot, TaskProgress, WriteSummary,
};
use super::coordinator_support::{apply_failure_code, state_name};
use super::domain::{ConfirmedResult, TaskState};
use super::infrastructure::{ApplyFailureReason, ApplyReport, Catalog, CatalogAsset};
use super::models::SmartCullingFaceModels;

pub(super) struct TaskSession {
    pub(super) task_id: String,
    pub(super) state: TaskState,
    pub(super) root_path: PathBuf,
    pub(super) mode: String,
    pub(super) device: DevicePreflight,
    pub(super) inventory: InventorySummary,
    pub(super) progress: TaskProgress,
    pub(super) catalog: Catalog,
    pub(super) models: Arc<SmartCullingFaceModels>,
    pub(super) cancellation: Arc<AtomicBool>,
    pub(super) results: Vec<ReviewResult>,
    pub(super) failures: Vec<FailureItem>,
    pub(super) detected_image_path: Option<String>,
    pub(super) detected_faces: Vec<DetectedFaceDto>,
    pub(super) assets: HashMap<String, CatalogAsset>,
    pub(super) pending_write: HashMap<PathBuf, ConfirmedResult>,
    pub(super) write_summary: Option<WriteSummary>,
    pub(super) started_at: Option<Instant>,
}

impl TaskSession {
    pub(super) fn transition_to(&mut self, next: TaskState) -> Result<(), String> {
        if !self.state.can_transition_to(next) {
            return Err(format!(
                "Invalid smart-culling task transition: {:?} -> {:?}",
                self.state, next
            ));
        }
        self.state = next;
        Ok(())
    }

    pub(super) fn snapshot(&self) -> SmartCullingSnapshot {
        SmartCullingSnapshot {
            revision: super::snapshot_order::next_revision(),
            task_id: Some(self.task_id.clone()),
            state: state_name(self.state).to_string(),
            root_path: Some(self.root_path.to_string_lossy().to_string()),
            mode: Some(self.mode.clone()),
            device: self.device.clone(),
            inventory: self.inventory.clone(),
            progress: self.progress.clone(),
            results: self.results.clone(),
            failures: self.failures.clone(),
            detected_image_path: self.detected_image_path.clone(),
            detected_faces: self.detected_faces.clone(),
            write_summary: self.write_summary.clone(),
            lock_change_summary: None,
        }
    }

    pub(super) fn apply_write_report(
        &mut self,
        report: ApplyReport,
        attempted: usize,
    ) -> Result<SmartCullingSnapshot, String> {
        let summary = self.write_summary.get_or_insert_with(|| WriteSummary {
            protected: self.inventory.protected_assets,
            skipped: self.inventory.skipped_assets,
            ..WriteSummary::default()
        });
        merge_write_report(
            &mut self.failures,
            &mut self.pending_write,
            &self.assets,
            summary,
            report,
        );
        self.transition_to(TaskState::Completed)?;
        if attempted == 0 {
            self.failures.push(FailureItem {
                path: self.root_path.to_string_lossy().to_string(),
                member_paths: Vec::new(),
                stage: "write".to_string(),
                code: "nothing_to_write".to_string(),
                detail: "No reviewed result had a valid catalog asset".to_string(),
                retryable: false,
            });
        }
        Ok(self.snapshot())
    }
}

fn merge_write_report(
    failures: &mut Vec<FailureItem>,
    pending_write: &mut HashMap<PathBuf, ConfirmedResult>,
    assets: &HashMap<String, CatalogAsset>,
    summary: &mut WriteSummary,
    report: ApplyReport,
) {
    // A retry reports only the remaining retryable assets. Preserve failures
    // for every asset outside this report, including permanent write failures.
    let attempted_paths = report
        .succeeded
        .iter()
        .chain(report.failed.iter().map(|failure| &failure.sidecar_path))
        .map(|path| path.to_string_lossy().to_string())
        .collect::<HashSet<_>>();
    failures.retain(|failure| failure.stage != "write" || !attempted_paths.contains(&failure.path));
    for succeeded in &report.succeeded {
        pending_write.remove(succeeded);
    }
    for failure in &report.failed {
        failures.push(FailureItem {
            path: failure.sidecar_path.to_string_lossy().to_string(),
            member_paths: assets
                .values()
                .find(|asset| asset.sidecar_path == failure.sidecar_path)
                .map(|asset| {
                    asset
                        .member_paths
                        .iter()
                        .map(|path| path.to_string_lossy().to_string())
                        .collect()
                })
                .unwrap_or_default(),
            stage: "write".to_string(),
            code: apply_failure_code(failure.reason).to_string(),
            detail: failure.detail.clone(),
            retryable: matches!(
                failure.reason,
                ApplyFailureReason::BaselineConflict | ApplyFailureReason::Io
            ),
        });
        if !matches!(
            failure.reason,
            ApplyFailureReason::BaselineConflict | ApplyFailureReason::Io
        ) {
            pending_write.remove(&failure.sidecar_path);
        }
    }
    summary
        .succeeded_paths
        .extend(report.succeeded.iter().filter_map(|sidecar| {
            assets
                .values()
                .find(|asset| asset.sidecar_path == *sidecar)
                .map(|asset| asset.display_path.to_string_lossy().to_string())
        }));
    summary.succeeded_paths.sort();
    summary.succeeded_paths.dedup();
    summary.succeeded += report.succeeded.len();
    summary.failed = failures
        .iter()
        .filter(|failure| failure.stage == "write")
        .count();
}

#[cfg(test)]
mod tests {
    use super::super::domain::ResultSource;
    use super::*;

    fn failure(path: &str, stage: &str, retryable: bool) -> FailureItem {
        FailureItem {
            path: path.to_string(),
            member_paths: Vec::new(),
            stage: stage.to_string(),
            code: if retryable {
                "write_io"
            } else {
                "asset_changed"
            }
            .to_string(),
            detail: "original failure detail".to_string(),
            retryable,
        }
    }

    fn pending_result() -> ConfirmedResult {
        ConfirmedResult {
            result_id: "retryable".to_string(),
            source: ResultSource::Manual,
            rating: 3,
            color_label: None,
            reason_codes: Vec::new(),
            confidence: 0.0,
            mode: "portrait".to_string(),
            model_version: "test".to_string(),
            policy_version: "test".to_string(),
            confirmed_at: "2026-10-28T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn retry_success_preserves_unattempted_permanent_and_analysis_failures() {
        let retry_path = PathBuf::from("/photos/retry.json");
        let mut failures = vec![
            failure("/photos/permanent.json", "write", false),
            failure("/photos/retry.json", "write", true),
            failure("/photos/unanalysed.jpg", "analysis", false),
        ];
        let mut pending = HashMap::from([(retry_path.clone(), pending_result())]);
        let mut summary = WriteSummary {
            succeeded: 2,
            failed: 2,
            protected: 3,
            skipped: 4,
            ..WriteSummary::default()
        };
        merge_write_report(
            &mut failures,
            &mut pending,
            &HashMap::new(),
            &mut summary,
            ApplyReport {
                succeeded: vec![retry_path],
                ..ApplyReport::default()
            },
        );

        assert_eq!(failures.len(), 2);
        assert_eq!(failures[0].path, "/photos/permanent.json");
        assert_eq!(failures[0].detail, "original failure detail");
        assert!(!failures[0].retryable);
        assert_eq!(failures[1].stage, "analysis");
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.succeeded, 3);
        assert_eq!((summary.protected, summary.skipped), (3, 4));
        assert!(pending.is_empty());
    }

    #[test]
    fn empty_report_does_not_clear_unattempted_write_failures() {
        let mut failures = vec![failure("/photos/retry.json", "write", true)];
        let mut summary = WriteSummary {
            failed: 1,
            ..WriteSummary::default()
        };
        merge_write_report(
            &mut failures,
            &mut HashMap::new(),
            &HashMap::new(),
            &mut summary,
            ApplyReport::default(),
        );
        assert_eq!(failures.len(), 1);
        assert!(failures[0].retryable);
        assert_eq!(summary.failed, 1);
    }

    #[test]
    fn retry_failure_replaces_its_old_error_without_losing_other_failures() {
        use super::super::infrastructure::{ConfirmedWrite, apply_confirmed_results};

        let retry_path = PathBuf::from("/photos/retry.json");
        let mut invalid = pending_result();
        invalid.rating = 6;
        let report = apply_confirmed_results(vec![ConfirmedWrite {
            sidecar_path: retry_path.clone(),
            member_sidecar_baselines: Vec::new(),
            file_baselines: Vec::new(),
            result: invalid,
        }]);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.failed[0].reason, ApplyFailureReason::InvalidResult);
        let mut failures = vec![
            failure("/photos/permanent.json", "write", false),
            failure("/photos/retry.json", "write", true),
        ];
        let mut pending = HashMap::from([(retry_path, pending_result())]);
        let mut summary = WriteSummary {
            failed: 2,
            ..WriteSummary::default()
        };
        merge_write_report(
            &mut failures,
            &mut pending,
            &HashMap::new(),
            &mut summary,
            report,
        );
        assert_eq!(failures.len(), 2);
        assert_eq!(failures[0].path, "/photos/permanent.json");
        assert_eq!(failures[1].code, "invalid_result");
        assert_eq!(failures[1].detail, "rating must be 0-5");
        assert!(!failures[1].retryable);
        assert!(pending.is_empty());
        assert_eq!(summary.failed, 2);
    }
}
