use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::Utc;
use once_cell::sync::Lazy;
use tauri::{AppHandle, Emitter};

use super::api::{
    DevicePreflight, FailureItem, InventorySummary, ReviewChange, SmartCullingRequest,
    SmartCullingSnapshot, TaskProgress, WriteSummary,
};
use super::coordinator_support::{
    apply_failure_code, catalog_failures, confirmed_result, eta_seconds, inventory_summary,
    mode_supports_key_people, state_name, valid_color, valid_key_people, valid_mode,
};
use super::domain::{ConfirmedResult, TaskState};
use super::infrastructure::{
    ApplyFailureReason, Catalog, CatalogAsset, CatalogAssetStatus, ConfirmedWrite,
    apply_confirmed_results, capture_sidecar_baseline, change_asset_lock_state,
    reconcile_manual_ownership, scan_catalog,
};
use super::models::SmartCullingFaceModels;
use super::preflight::run_preflight;
use super::runner::{detect_people, run_analysis};

const EVENT_NAME: &str = "smart-culling://event";

static COORDINATOR: Lazy<Mutex<Coordinator>> = Lazy::new(|| Mutex::new(Coordinator::default()));

#[derive(Default)]
struct Coordinator {
    session: Option<TaskSession>,
    last_snapshot: SmartCullingSnapshot,
}

struct TaskSession {
    task_id: String,
    state: TaskState,
    root_path: PathBuf,
    mode: String,
    device: DevicePreflight,
    inventory: InventorySummary,
    progress: TaskProgress,
    catalog: Catalog,
    models: Arc<SmartCullingFaceModels>,
    cancellation: Arc<AtomicBool>,
    results: Vec<super::api::ReviewResult>,
    failures: Vec<FailureItem>,
    detected_image_path: Option<String>,
    detected_faces: Vec<super::api::DetectedFaceDto>,
    assets: HashMap<String, CatalogAsset>,
    pending_write: HashMap<PathBuf, ConfirmedResult>,
    write_summary: Option<WriteSummary>,
    started_at: Option<Instant>,
}

pub(crate) fn handle(
    request: SmartCullingRequest,
    app_handle: AppHandle,
) -> Result<SmartCullingSnapshot, String> {
    match request {
        SmartCullingRequest::Status => Ok(current_snapshot()),
        SmartCullingRequest::Inspect { root_path } => inspect(root_path, &app_handle),
        SmartCullingRequest::DetectPeople { path } => detect_people_request(path, &app_handle),
        SmartCullingRequest::Start {
            root_path,
            mode,
            key_people,
        } => start(root_path, mode, key_people, app_handle),
        SmartCullingRequest::Cancel => cancel(&app_handle),
        SmartCullingRequest::UpdateReview { changes } => update_review(changes, &app_handle),
        SmartCullingRequest::Confirm => confirm(&app_handle),
        SmartCullingRequest::RetryFailures => retry_failures(&app_handle),
        SmartCullingRequest::ReconcileManual { paths } => reconcile_manual(paths, &app_handle),
        SmartCullingRequest::SetLock { paths, locked } => set_lock(paths, locked, &app_handle),
        SmartCullingRequest::Abandon => abandon(&app_handle),
    }
}

fn inspect(root_path: String, app_handle: &AppHandle) -> Result<SmartCullingSnapshot, String> {
    let root = PathBuf::from(&root_path);
    {
        let coordinator = COORDINATOR.lock().unwrap();
        if let Some(session) = coordinator.session.as_ref()
            && matches!(
                session.state,
                TaskState::Indexing
                    | TaskState::Rendering
                    | TaskState::Analyzing
                    | TaskState::Organizing
                    | TaskState::Cancelling
                    | TaskState::ReadyForReview
                    | TaskState::Confirming
            )
        {
            return Ok(snapshot_for(session));
        }
    }

    let (device, models) = match run_preflight(app_handle) {
        Ok(value) => value,
        Err(device) => {
            let snapshot = SmartCullingSnapshot {
                state: "unsupported".to_string(),
                root_path: Some(root_path),
                device,
                ..SmartCullingSnapshot::default()
            };
            COORDINATOR.lock().unwrap().last_snapshot = snapshot.clone();
            emit_snapshot(app_handle, &snapshot);
            return Ok(snapshot);
        }
    };
    let catalog = scan_catalog(&root)?;
    let inventory = inventory_summary(&catalog);
    let failures = catalog_failures(&catalog);
    let session = TaskSession {
        task_id: uuid::Uuid::new_v4().to_string(),
        state: TaskState::Configuring,
        root_path: root,
        mode: "auto".to_string(),
        device,
        inventory,
        progress: TaskProgress::default(),
        catalog,
        models,
        cancellation: Arc::new(AtomicBool::new(false)),
        results: Vec::new(),
        failures,
        detected_image_path: None,
        detected_faces: Vec::new(),
        assets: HashMap::new(),
        pending_write: HashMap::new(),
        write_summary: None,
        started_at: None,
    };
    let snapshot = snapshot_for(&session);
    let mut coordinator = COORDINATOR.lock().unwrap();
    coordinator.last_snapshot = snapshot.clone();
    coordinator.session = Some(session);
    drop(coordinator);
    emit_snapshot(app_handle, &snapshot);
    Ok(snapshot)
}

fn detect_people_request(
    path: String,
    app_handle: &AppHandle,
) -> Result<SmartCullingSnapshot, String> {
    let models = {
        let coordinator = COORDINATOR.lock().unwrap();
        let session = coordinator
            .session
            .as_ref()
            .ok_or_else(|| "Inspect a folder before selecting key people".to_string())?;
        if session.state != TaskState::Configuring {
            return Err("Key people can only be selected before analysis starts".to_string());
        }
        if !session.catalog.assets.iter().any(|asset| {
            asset
                .member_paths
                .iter()
                .any(|member| member == std::path::Path::new(&path))
        }) {
            return Err("The selected key-person photo is outside this task".to_string());
        }
        session.models.clone()
    };
    let faces = detect_people(app_handle, &path, &models)?;
    let snapshot = {
        let mut coordinator = COORDINATOR.lock().unwrap();
        let session = coordinator.session.as_mut().unwrap();
        session.detected_image_path = Some(path);
        session.detected_faces = faces;
        snapshot_for(session)
    };
    save_and_emit(app_handle, snapshot.clone());
    Ok(snapshot)
}

fn start(
    root_path: String,
    mode: String,
    key_people: Vec<super::api::KeyPersonSelection>,
    app_handle: AppHandle,
) -> Result<SmartCullingSnapshot, String> {
    if !valid_mode(&mode) {
        return Err(format!("Unknown smart-culling mode: {mode}"));
    }
    if !key_people.is_empty() && !mode_supports_key_people(&mode) {
        return Err(format!(
            "Key people are not supported in smart-culling mode: {mode}"
        ));
    }

    let (task_id, root, assets, models, cancellation) = {
        let mut coordinator = COORDINATOR.lock().unwrap();
        let session = coordinator
            .session
            .as_mut()
            .ok_or_else(|| "Inspect a folder before starting smart culling".to_string())?;
        if session.root_path != PathBuf::from(&root_path) {
            return Err(
                "The configured folder changed; inspect it again before starting".to_string(),
            );
        }
        if session.state != TaskState::Configuring {
            return Err("Only one smart-culling task can run or wait for review".to_string());
        }
        if !valid_key_people(&key_people, &session.catalog) {
            return Err("Key-person selections are invalid or outside this task".to_string());
        }
        session.mode = mode.clone();
        session.state = TaskState::Indexing;
        session.started_at = Some(Instant::now());
        session.detected_image_path = None;
        session.detected_faces.clear();
        session.progress = TaskProgress {
            completed: 0,
            total: session.inventory.eligible_assets,
            percent: 0,
            stage: "indexing".to_string(),
            eta_seconds: None,
            partial: false,
        };
        let eligible = session
            .catalog
            .assets
            .iter()
            .filter(|asset| asset.status == CatalogAssetStatus::Eligible)
            .cloned()
            .collect::<Vec<_>>();
        (
            session.task_id.clone(),
            session.root_path.clone(),
            eligible,
            session.models.clone(),
            session.cancellation.clone(),
        )
    };

    let initial = current_snapshot();
    save_and_emit(&app_handle, initial.clone());
    let thread_app_handle = app_handle.clone();
    if let Err(error) = std::thread::Builder::new()
        .name("smart-culling-v2".to_string())
        .spawn(move || {
            update_running_state(
                &task_id,
                TaskState::Rendering,
                "rendering",
                &thread_app_handle,
            );
            let outcome = run_analysis(
                &thread_app_handle,
                &root,
                &mode,
                assets,
                &key_people,
                &models,
                &cancellation,
                |completed, total, stage| {
                    update_progress(&task_id, completed, total, stage, &thread_app_handle)
                },
            );
            finish_analysis(&task_id, outcome, &thread_app_handle);
        })
    {
        let snapshot = restore_configuring_after_start_failure();
        save_and_emit(&app_handle, snapshot);
        return Err(error.to_string());
    }
    Ok(initial)
}

fn cancel(app_handle: &AppHandle) -> Result<SmartCullingSnapshot, String> {
    let snapshot = {
        let mut coordinator = COORDINATOR.lock().unwrap();
        let session = coordinator
            .session
            .as_mut()
            .ok_or_else(|| "There is no smart-culling task to cancel".to_string())?;
        if !matches!(
            session.state,
            TaskState::Indexing
                | TaskState::Rendering
                | TaskState::Analyzing
                | TaskState::Organizing
        ) {
            return Err("The current task is not running".to_string());
        }
        session.state = TaskState::Cancelling;
        session.cancellation.store(true, Ordering::Release);
        session.progress.stage = "cancelling".to_string();
        snapshot_for(session)
    };
    save_and_emit(app_handle, snapshot.clone());
    Ok(snapshot)
}

fn update_review(
    changes: Vec<ReviewChange>,
    app_handle: &AppHandle,
) -> Result<SmartCullingSnapshot, String> {
    let snapshot = {
        let mut coordinator = COORDINATOR.lock().unwrap();
        let session = coordinator
            .session
            .as_mut()
            .ok_or_else(|| "There is no result waiting for review".to_string())?;
        if session.state != TaskState::ReadyForReview {
            return Err("Review changes are accepted only on the review page".to_string());
        }
        for change in changes {
            if change.rating > 5
                || !valid_color(change.color_label.as_deref())
                || !valid_mode(&change.mode)
            {
                return Err("Review rating or color label is invalid".to_string());
            }
            let Some(result) = session
                .results
                .iter_mut()
                .find(|result| result.result_id == change.result_id)
            else {
                continue;
            };
            result.adopted = change.adopted;
            if change.metadata_edited {
                result.rating = change.rating;
                result.color_label = change.color_label;
                result.source = "manual".to_string();
                result.reason_codes.clear();
                result.confidence = 0.0;
                result.protected = true;
                result.requires_human_review = false;
            }
            if change.mode_changed {
                result.mode = change.mode;
                if result.source == "ai" {
                    result.reason_codes = vec!["mode_corrected_review".to_string()];
                    result.confidence = 0.0;
                    result.color_label = Some("yellow".to_string());
                }
            }
        }
        snapshot_for(session)
    };
    save_and_emit(app_handle, snapshot.clone());
    Ok(snapshot)
}

fn confirm(app_handle: &AppHandle) -> Result<SmartCullingSnapshot, String> {
    let (items, adopted_count) = {
        let mut coordinator = COORDINATOR.lock().unwrap();
        let session = coordinator
            .session
            .as_mut()
            .ok_or_else(|| "There is no result waiting for confirmation".to_string())?;
        if session.state != TaskState::ReadyForReview {
            return Err("Results must be reviewed before confirmation".to_string());
        }
        let adopted = session
            .results
            .iter()
            .filter(|result| result.adopted)
            .cloned()
            .collect::<Vec<_>>();
        if adopted.is_empty() {
            coordinator.session = None;
            coordinator.last_snapshot = SmartCullingSnapshot::default();
            let snapshot = coordinator.last_snapshot.clone();
            drop(coordinator);
            emit_snapshot(app_handle, &snapshot);
            return Ok(snapshot);
        }
        session.state = TaskState::Confirming;
        let confirmed_at = Utc::now().to_rfc3339();
        session.pending_write.clear();
        let mut items = Vec::with_capacity(adopted.len());
        for result in adopted {
            let Some(asset) = session.assets.get(&result.result_id) else {
                continue;
            };
            let confirmed = confirmed_result(&result, &confirmed_at)?;
            session
                .pending_write
                .insert(asset.sidecar_path.clone(), confirmed.clone());
            items.push(ConfirmedWrite {
                sidecar_path: asset.sidecar_path.clone(),
                member_sidecar_baselines: asset.member_sidecar_baselines.clone(),
                file_baselines: asset.file_baselines.clone(),
                result: confirmed,
            });
        }
        (items, session.pending_write.len())
    };

    let report = apply_confirmed_results(items);
    finish_write(report, adopted_count, app_handle)
}

fn retry_failures(app_handle: &AppHandle) -> Result<SmartCullingSnapshot, String> {
    let items = {
        let coordinator = COORDINATOR.lock().unwrap();
        let session = coordinator
            .session
            .as_ref()
            .ok_or_else(|| "There is no completed task to retry".to_string())?;
        if session.state != TaskState::Completed || session.pending_write.is_empty() {
            return Err("There are no retryable write failures".to_string());
        }
        session
            .pending_write
            .iter()
            .filter_map(|(sidecar, result)| {
                session
                    .assets
                    .values()
                    .find(|asset| asset.sidecar_path == *sidecar)
                    .map(|asset| {
                        let member_sidecar_baselines = asset
                            .member_sidecar_baselines
                            .iter()
                            .map(|(path, baseline)| {
                                (
                                    path.clone(),
                                    capture_sidecar_baseline(path)
                                        .unwrap_or_else(|_| baseline.clone()),
                                )
                            })
                            .collect();
                        ConfirmedWrite {
                            sidecar_path: sidecar.clone(),
                            member_sidecar_baselines,
                            file_baselines: asset.file_baselines.clone(),
                            result: result.clone(),
                        }
                    })
            })
            .collect::<Vec<_>>()
    };
    let pending = items.len();
    let report = apply_confirmed_results(items);
    finish_write(report, pending, app_handle)
}

fn abandon(app_handle: &AppHandle) -> Result<SmartCullingSnapshot, String> {
    let snapshot = {
        let mut coordinator = COORDINATOR.lock().unwrap();
        if let Some(session) = coordinator.session.as_ref()
            && matches!(
                session.state,
                TaskState::Indexing
                    | TaskState::Rendering
                    | TaskState::Analyzing
                    | TaskState::Organizing
                    | TaskState::Cancelling
                    | TaskState::Confirming
            )
        {
            return Err("Cancel the running task before abandoning it".to_string());
        }
        coordinator.session = None;
        coordinator.last_snapshot = SmartCullingSnapshot::default();
        coordinator.last_snapshot.clone()
    };
    emit_snapshot(app_handle, &snapshot);
    Ok(snapshot)
}

fn restore_configuring_after_start_failure() -> SmartCullingSnapshot {
    let mut coordinator = COORDINATOR.lock().unwrap();
    let Some(session) = coordinator.session.as_mut() else {
        return coordinator.last_snapshot.clone();
    };
    session.state = TaskState::Configuring;
    session.started_at = None;
    session.cancellation.store(false, Ordering::Release);
    session.progress = TaskProgress::default();
    snapshot_for(session)
}

fn reconcile_manual(
    paths: Vec<String>,
    app_handle: &AppHandle,
) -> Result<SmartCullingSnapshot, String> {
    let report = reconcile_manual_ownership(paths.into_iter().map(PathBuf::from).collect());
    if let Some(failure) = report.failed.first() {
        log::warn!(
            "Failed to reconcile manual smart-culling ownership for {}: {}",
            failure.sidecar_path.display(),
            failure.detail
        );
        return Err(format!(
            "Could not protect the complete RAW/JPEG asset at {}: {}",
            failure.sidecar_path.display(),
            failure.detail
        ));
    }
    let snapshot = current_snapshot();
    emit_snapshot(app_handle, &snapshot);
    Ok(snapshot)
}

fn set_lock(
    paths: Vec<String>,
    locked: bool,
    app_handle: &AppHandle,
) -> Result<SmartCullingSnapshot, String> {
    change_asset_lock_state(paths.into_iter().map(PathBuf::from).collect(), locked)?;
    let snapshot = current_snapshot();
    emit_snapshot(app_handle, &snapshot);
    Ok(snapshot)
}

fn finish_write(
    report: super::infrastructure::ApplyReport,
    attempted: usize,
    app_handle: &AppHandle,
) -> Result<SmartCullingSnapshot, String> {
    let snapshot = {
        let mut coordinator = COORDINATOR.lock().unwrap();
        let session = coordinator
            .session
            .as_mut()
            .ok_or_else(|| "Smart-culling task disappeared while writing".to_string())?;
        session.failures.retain(|failure| failure.stage != "write");
        for succeeded in &report.succeeded {
            session.pending_write.remove(succeeded);
        }
        for failure in &report.failed {
            session.failures.push(FailureItem {
                path: failure.sidecar_path.to_string_lossy().to_string(),
                member_paths: session
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
                session.pending_write.remove(&failure.sidecar_path);
            }
        }
        let mut succeeded_paths = session
            .write_summary
            .as_ref()
            .map(|summary| summary.succeeded_paths.clone())
            .unwrap_or_default();
        succeeded_paths.extend(
            report
                .succeeded
                .iter()
                .filter_map(|sidecar| {
                    session
                        .assets
                        .values()
                        .find(|asset| asset.sidecar_path == *sidecar)
                        .map(|asset| asset.display_path.to_string_lossy().to_string())
                })
                .collect::<Vec<_>>(),
        );
        succeeded_paths.sort();
        succeeded_paths.dedup();
        let previous_succeeded = session
            .write_summary
            .as_ref()
            .map(|summary| summary.succeeded)
            .unwrap_or(0);
        session.write_summary = Some(WriteSummary {
            succeeded: previous_succeeded + report.succeeded.len(),
            failed: report.failed.len(),
            protected: session.inventory.protected_assets,
            skipped: session.inventory.skipped_assets,
            succeeded_paths,
        });
        session.state = TaskState::Completed;
        if attempted == 0 {
            session.failures.push(FailureItem {
                path: session.root_path.to_string_lossy().to_string(),
                member_paths: Vec::new(),
                stage: "write".to_string(),
                code: "nothing_to_write".to_string(),
                detail: "No adopted result had a valid catalog asset".to_string(),
                retryable: false,
            });
        }
        snapshot_for(session)
    };
    save_and_emit(app_handle, snapshot.clone());
    Ok(snapshot)
}

fn update_running_state(task_id: &str, state: TaskState, stage: &str, app_handle: &AppHandle) {
    let snapshot = {
        let mut coordinator = COORDINATOR.lock().unwrap();
        let Some(session) = coordinator.session.as_mut() else {
            return;
        };
        if session.task_id != task_id || session.cancellation.load(Ordering::Acquire) {
            return;
        }
        session.state = state;
        session.progress.stage = stage.to_string();
        snapshot_for(session)
    };
    save_and_emit(app_handle, snapshot);
}

fn update_progress(
    task_id: &str,
    completed: usize,
    total: usize,
    stage: &str,
    app_handle: &AppHandle,
) {
    let snapshot = {
        let mut coordinator = COORDINATOR.lock().unwrap();
        let Some(session) = coordinator.session.as_mut() else {
            return;
        };
        if session.task_id != task_id {
            return;
        }
        if session.state != TaskState::Cancelling {
            session.state = if stage == "organizing" {
                TaskState::Organizing
            } else {
                TaskState::Analyzing
            };
        }
        session.progress.completed = completed;
        session.progress.total = total;
        session.progress.percent = if total == 0 {
            100
        } else {
            ((completed * 100) / total).min(100) as u8
        };
        session.progress.stage = stage.to_string();
        session.progress.eta_seconds = eta_seconds(session.started_at, completed, total);
        snapshot_for(session)
    };
    save_and_emit(app_handle, snapshot);
}

fn finish_analysis(task_id: &str, outcome: super::runner::RunOutcome, app_handle: &AppHandle) {
    let snapshot = {
        let mut coordinator = COORDINATOR.lock().unwrap();
        let Some(session) = coordinator.session.as_mut() else {
            return;
        };
        if session.task_id != task_id {
            return;
        }
        session.results = outcome.results;
        session.assets = outcome.assets;
        session.failures.extend(outcome.failures);
        session.progress.completed = outcome.completed;
        session.progress.total = outcome.total;
        session.progress.percent = if outcome.total == 0 {
            100
        } else {
            ((outcome.completed * 100) / outcome.total).min(100) as u8
        };
        session.progress.stage = "readyForReview".to_string();
        session.progress.eta_seconds = None;
        session.progress.partial = outcome.partial;
        session.state = TaskState::ReadyForReview;
        snapshot_for(session)
    };
    save_and_emit(app_handle, snapshot);
}

fn current_snapshot() -> SmartCullingSnapshot {
    let coordinator = COORDINATOR.lock().unwrap();
    coordinator
        .session
        .as_ref()
        .map(snapshot_for)
        .unwrap_or_else(|| coordinator.last_snapshot.clone())
}

fn snapshot_for(session: &TaskSession) -> SmartCullingSnapshot {
    SmartCullingSnapshot {
        task_id: Some(session.task_id.clone()),
        state: state_name(session.state).to_string(),
        root_path: Some(session.root_path.to_string_lossy().to_string()),
        mode: Some(session.mode.clone()),
        device: session.device.clone(),
        inventory: session.inventory.clone(),
        progress: session.progress.clone(),
        results: session.results.clone(),
        failures: session.failures.clone(),
        detected_image_path: session.detected_image_path.clone(),
        detected_faces: session.detected_faces.clone(),
        write_summary: session.write_summary.clone(),
    }
}

fn save_and_emit(app_handle: &AppHandle, snapshot: SmartCullingSnapshot) {
    COORDINATOR.lock().unwrap().last_snapshot = snapshot.clone();
    emit_snapshot(app_handle, &snapshot);
}

fn emit_snapshot(app_handle: &AppHandle, snapshot: &SmartCullingSnapshot) {
    if let Err(error) = app_handle.emit(EVENT_NAME, snapshot) {
        log::warn!("Failed to emit smart-culling snapshot: {error}");
    }
}
