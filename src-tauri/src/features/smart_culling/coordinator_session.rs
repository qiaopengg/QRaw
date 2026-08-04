use std::collections::HashMap;
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
    pub(super) fn snapshot(&self) -> SmartCullingSnapshot {
        SmartCullingSnapshot {
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
    ) -> SmartCullingSnapshot {
        self.failures.retain(|failure| failure.stage != "write");
        for succeeded in &report.succeeded {
            self.pending_write.remove(succeeded);
        }
        for failure in &report.failed {
            self.failures.push(FailureItem {
                path: failure.sidecar_path.to_string_lossy().to_string(),
                member_paths: self
                    .assets
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
                self.pending_write.remove(&failure.sidecar_path);
            }
        }
        let mut succeeded_paths = self
            .write_summary
            .as_ref()
            .map(|summary| summary.succeeded_paths.clone())
            .unwrap_or_default();
        succeeded_paths.extend(report.succeeded.iter().filter_map(|sidecar| {
            self.assets
                .values()
                .find(|asset| asset.sidecar_path == *sidecar)
                .map(|asset| asset.display_path.to_string_lossy().to_string())
        }));
        succeeded_paths.sort();
        succeeded_paths.dedup();
        let previous_succeeded = self
            .write_summary
            .as_ref()
            .map(|summary| summary.succeeded)
            .unwrap_or(0);
        self.write_summary = Some(WriteSummary {
            succeeded: previous_succeeded + report.succeeded.len(),
            failed: report.failed.len(),
            protected: self.inventory.protected_assets,
            skipped: self.inventory.skipped_assets,
            succeeded_paths,
        });
        self.state = TaskState::Completed;
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
        self.snapshot()
    }
}
