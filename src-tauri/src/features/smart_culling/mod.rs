use std::{
    collections::{HashMap, VecDeque},
    fs, io,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use ab_glyph::{FontArc, PxScale};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, GenericImageView, GrayImage, imageops};
use image::{Rgb, RgbImage, Rgba, RgbaImage};
use image_hasher::{HashAlg, HasherConfig, ImageHash};
use imageproc::drawing::draw_text_mut;
use once_cell::sync::Lazy;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::{
    app_settings::load_settings,
    file_management::{parse_virtual_path, sync_metadata_to_xmp},
    formats::{is_raw_file, is_supported_image_file},
    image_loader,
    image_processing::ImageMetadata,
    tagging::COLOR_TAG_PREFIX,
};

static TASKS: Lazy<Mutex<HashMap<String, StoredTask>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static ACTIVE_TASK: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

const SHARED_CLIP_MODEL_FILENAME: &str = "clip_model.onnx";
const SHARED_CLIP_TOKENIZER_FILENAME: &str = "clip_tokenizer.json";
const SHARED_CLIP_MODEL_SHA256: &str =
    "57879bb1c23cdeb350d23569dd251ed4b740a96d747c529e94a2bb8040ac5d00";
const SHARED_CLIP_TOKENIZER_SHA256: &str =
    "b556ac8c99757ffb677208af34bc8c6721572114111a6e0aaf5fa69ff0b8d842";
const MIRROR_CLIP_MODEL_URL: &str =
    "https://hf-mirror.com/CyberTimon/RapidRAW-Models/resolve/main/clip_model.onnx?download=true";
const MIRROR_CLIP_TOKENIZER_URL: &str = "https://hf-mirror.com/CyberTimon/RapidRAW-Models/resolve/main/clip_tokenizer.json?download=true";
const FACE_DETECTOR_FILENAME: &str = "face_detection_yunet_2023mar.onnx";
const EMOTION_MODEL_FILENAME: &str = "emotion-ferplus-8.onnx";
const AESTHETIC_HEAD_FILENAME: &str = "smart-culling/aesthetic_head.onnx";
const RECENT_TASK_LIMIT: usize = 10;

#[derive(Clone)]
struct StoredTask {
    cancel: Arc<AtomicBool>,
    result: Option<SmartCullingTaskResult>,
    status: String,
    error: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingStartParams {
    pub paths: Vec<String>,
    pub mode: String,
    pub preset: String,
    #[serde(default = "default_aesthetic_preference")]
    pub aesthetic_preference: String,
    #[serde(default)]
    pub face_checks: Vec<String>,
    pub include_edited: bool,
    pub preview_only: bool,
    pub keep_per_group: usize,
    pub face_analysis_enabled: bool,
    pub allow_degraded: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingPresetConfig {
    pub mode: String,
    pub preset: String,
    #[serde(default = "default_aesthetic_preference")]
    pub aesthetic_preference: String,
    #[serde(default)]
    pub include_edited: bool,
    #[serde(default)]
    pub preview_only: bool,
    #[serde(default = "default_keep_per_group")]
    pub keep_per_group: usize,
    #[serde(default)]
    pub face_analysis_enabled: bool,
    #[serde(default)]
    pub face_checks: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingUserPreset {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub config: SmartCullingPresetConfig,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingSavePresetParams {
    pub id: Option<String>,
    pub name: String,
    pub config: SmartCullingPresetConfig,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingStartResponse {
    pub task_id: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingModelsStatus {
    pub models_dir: String,
    pub manifest_found: bool,
    pub can_run_full: bool,
    pub can_run_basic: bool,
    pub degraded_reason: Option<String>,
    pub missing_required: Vec<String>,
    pub missing_optional: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingProgress {
    pub task_id: String,
    pub current: usize,
    pub total: usize,
    pub stage: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingSummary {
    pub analyzed: usize,
    pub skipped: usize,
    pub selected: usize,
    pub review: usize,
    pub reject_suggestion: usize,
    pub failed: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingReviewItem {
    pub path: String,
    pub file_name: String,
    pub rating: u8,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_label: Option<String>,
    pub score: f64,
    pub confidence: f64,
    pub degraded: bool,
    pub reason_codes: Vec<String>,
    pub reason_text: String,
    pub group_id: Option<String>,
    pub group_rank: Option<usize>,
    pub group_size: Option<usize>,
    pub skip_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingTaskResult {
    pub task_id: String,
    pub status: String,
    pub preview_only: bool,
    pub degraded: bool,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub summary: SmartCullingSummary,
    pub items: Vec<SmartCullingReviewItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingApplyResult {
    pub task_id: String,
    pub applied: usize,
    pub skipped: usize,
    pub applied_paths: Vec<String>,
    pub skipped_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingHistoryItem {
    pub task_id: String,
    pub status: String,
    pub created_at: String,
    pub summary: SmartCullingSummary,
    pub degraded: bool,
    pub preview_only: bool,
    pub report_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingExportReportParams {
    pub task_id: String,
    pub items: Vec<SmartCullingReviewItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingReportResult {
    pub task_id: String,
    pub report_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingUndoResult {
    pub task_id: String,
    pub restored: usize,
    pub skipped: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SmartCullingAppliedSnapshot {
    path: String,
    sidecar_path: String,
    previous_metadata: ImageMetadata,
    applied_metadata: ImageMetadata,
}

struct SmartCullingAsset {
    display_path: String,
    source_path: PathBuf,
    file_name: String,
    skip_reason: Option<String>,
}

struct ImageAnalysisData {
    hash: ImageHash,
    item: SmartCullingReviewItem,
}

fn default_aesthetic_preference() -> String {
    "general".to_string()
}

fn default_keep_per_group() -> usize {
    1
}

pub fn smart_culling_check_models(
    app_handle: AppHandle,
) -> Result<SmartCullingModelsStatus, String> {
    let shared_models_dir = shared_models_dir(&app_handle)?;
    let models_dir = smart_culling_models_dir(&app_handle)?;
    let manifest_path = models_dir.join("manifest.json");
    let model_path = shared_models_dir.join(SHARED_CLIP_MODEL_FILENAME);
    let tokenizer_path = shared_models_dir.join(SHARED_CLIP_TOKENIZER_FILENAME);
    let model_found = verify_sha256(&model_path, SHARED_CLIP_MODEL_SHA256).unwrap_or(false);
    let tokenizer_found =
        verify_sha256(&tokenizer_path, SHARED_CLIP_TOKENIZER_SHA256).unwrap_or(false);
    let face_detector_found = shared_models_dir.join(FACE_DETECTOR_FILENAME).exists();
    let emotion_model_found = shared_models_dir.join(EMOTION_MODEL_FILENAME).exists();
    let aesthetic_head_found = shared_models_dir.join(AESTHETIC_HEAD_FILENAME).exists();
    let can_run_full = model_found && tokenizer_found;
    let mut manifest_found = manifest_is_valid(&manifest_path);

    if can_run_full && !manifest_found {
        write_shared_clip_manifest(&app_handle)?;
        manifest_found = true;
    }

    let mut missing_required = Vec::new();
    if !model_found {
        missing_required.push(SHARED_CLIP_MODEL_FILENAME.to_string());
    }
    if !tokenizer_found {
        missing_required.push(SHARED_CLIP_TOKENIZER_FILENAME.to_string());
    }

    Ok(SmartCullingModelsStatus {
        models_dir: shared_models_dir.to_string_lossy().to_string(),
        manifest_found,
        can_run_full,
        can_run_basic: true,
        degraded_reason: if can_run_full {
            None
        } else {
            Some("未检测到 CLIP 模型与 tokenizer，将使用基础本地分析模式。".to_string())
        },
        missing_required,
        missing_optional: {
            let mut optional = Vec::new();
            if !manifest_found {
                optional.push("smart-culling/manifest.json".to_string());
            }
            if !aesthetic_head_found {
                optional.push(AESTHETIC_HEAD_FILENAME.to_string());
            }
            if !face_detector_found {
                optional.push(FACE_DETECTOR_FILENAME.to_string());
            }
            if !emotion_model_found {
                optional.push(EMOTION_MODEL_FILENAME.to_string());
            }
            optional
        },
    })
}

pub async fn smart_culling_download_models(
    app_handle: AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<SmartCullingModelsStatus, String> {
    let _ = app_handle.emit(
        "smart-culling:model-download-start",
        json!({ "stage": "准备下载智能选图模型" }),
    );

    let result = match crate::ai_processing::get_or_init_clip_models(
        &app_handle,
        &state.ai_state,
        &state.ai_init_lock,
    )
    .await
    {
        Ok(models) => Ok(models),
        Err(primary_error) => {
            let primary_message = primary_error.to_string();
            let _ = app_handle.emit(
                "smart-culling:model-download-start",
                json!({ "stage": "主下载源不可用，尝试备用镜像" }),
            );
            match download_shared_clip_from_mirror(&app_handle).await {
                Ok(()) => crate::ai_processing::get_or_init_clip_models(
                    &app_handle,
                    &state.ai_state,
                    &state.ai_init_lock,
                )
                .await
                .map_err(|error| {
                    format!(
                        "主下载失败：{}；备用镜像下载后初始化失败：{}",
                        primary_message, error
                    )
                }),
                Err(fallback_error) => Err(format!(
                    "主下载失败：{}；备用镜像失败：{}",
                    primary_message, fallback_error
                )),
            }
        }
    };

    match result {
        Ok(_) => {
            write_shared_clip_manifest(&app_handle)?;
            let status = smart_culling_check_models(app_handle.clone())?;
            let _ = app_handle.emit("smart-culling:model-download-finish", status.clone());
            Ok(status)
        }
        Err(error) => {
            let message = error.to_string();
            let _ = app_handle.emit(
                "smart-culling:model-download-failed",
                json!({ "error": message }),
            );
            Err(message)
        }
    }
}

pub fn smart_culling_open_models_dir(app_handle: AppHandle) -> Result<String, String> {
    let models_dir = shared_models_dir(&app_handle)?;
    Ok(models_dir.to_string_lossy().to_string())
}

pub async fn smart_culling_start_task(
    params: SmartCullingStartParams,
    app_handle: AppHandle,
) -> Result<SmartCullingStartResponse, String> {
    if params.paths.is_empty() {
        return Err("没有可分析的图片。".to_string());
    }

    {
        let active = ACTIVE_TASK.lock().map_err(|e| e.to_string())?;
        if active.is_some() {
            return Err("已有智能选图任务正在运行。".to_string());
        }
    }

    let task_id = Uuid::new_v4().to_string();
    let cancel = Arc::new(AtomicBool::new(false));

    {
        let mut tasks = TASKS.lock().map_err(|e| e.to_string())?;
        tasks.insert(
            task_id.clone(),
            StoredTask {
                cancel: Arc::clone(&cancel),
                result: None,
                status: "running".to_string(),
                error: None,
            },
        );
    }
    *ACTIVE_TASK.lock().map_err(|e| e.to_string())? = Some(task_id.clone());

    let worker_task_id = task_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let failure_params = params.clone();
        let result = run_task(
            worker_task_id.clone(),
            params,
            app_handle.clone(),
            Arc::clone(&cancel),
        );
        let mut tasks = TASKS.lock().unwrap();
        if let Some(stored) = tasks.get_mut(&worker_task_id) {
            match result {
                Ok(task_result) => {
                    stored.status = task_result.status.clone();
                    stored.result = Some(task_result.clone());
                    if let Err(error) = persist_task_result(&app_handle, &task_result) {
                        eprintln!("Failed to persist smart culling task: {}", error);
                    }
                    if let Err(error) = prune_recent_tasks(&app_handle) {
                        eprintln!("Failed to prune smart culling task history: {}", error);
                    }
                    let _ = app_handle.emit("smart-culling:review-ready", task_result);
                }
                Err(error) => {
                    stored.status = if cancel.load(Ordering::SeqCst) {
                        "cancelled".to_string()
                    } else {
                        "failed".to_string()
                    };
                    stored.error = Some(error.clone());
                    if stored.status == "failed" {
                        let mut failure_result = build_failed_task_result(
                            worker_task_id.clone(),
                            &failure_params,
                            &error,
                        );
                        if let Err(report_error) =
                            persist_task_report_pdf(&app_handle, &mut failure_result)
                        {
                            eprintln!(
                                "Failed to persist smart culling failure report: {}",
                                report_error
                            );
                        }
                        if let Err(persist_error) =
                            persist_task_result(&app_handle, &failure_result)
                        {
                            eprintln!(
                                "Failed to persist smart culling failure task: {}",
                                persist_error
                            );
                        }
                        if let Err(prune_error) = prune_recent_tasks(&app_handle) {
                            eprintln!(
                                "Failed to prune smart culling task history: {}",
                                prune_error
                            );
                        }
                        stored.result = Some(failure_result.clone());
                    }
                    let event = if stored.status == "cancelled" {
                        "smart-culling:cancelled"
                    } else {
                        "smart-culling:failed"
                    };
                    let _ =
                        app_handle.emit(event, json!({ "taskId": worker_task_id, "error": error }));
                }
            }
        }
        *ACTIVE_TASK.lock().unwrap() = None;
    });

    Ok(SmartCullingStartResponse { task_id })
}

pub fn smart_culling_cancel_task(task_id: String) -> Result<(), String> {
    let tasks = TASKS.lock().map_err(|e| e.to_string())?;
    let Some(task) = tasks.get(&task_id) else {
        return Err("未找到智能选图任务。".to_string());
    };
    task.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

pub fn smart_culling_get_task_result(
    task_id: String,
    app_handle: AppHandle,
) -> Result<SmartCullingTaskResult, String> {
    let tasks = TASKS.lock().map_err(|e| e.to_string())?;
    if let Some(task) = tasks.get(&task_id) {
        return task.result.clone().ok_or_else(|| {
            task.error
                .clone()
                .unwrap_or_else(|| "任务尚未生成复核结果。".to_string())
        });
    }
    drop(tasks);
    load_task_result(&app_handle, &task_id)
}

pub fn smart_culling_discard_task_result(
    task_id: String,
    app_handle: AppHandle,
) -> Result<(), String> {
    let mut tasks = TASKS.lock().map_err(|e| e.to_string())?;
    if let Some(task) = tasks.get_mut(&task_id) {
        task.status = "discarded".to_string();
        if let Some(result) = &mut task.result {
            result.status = "discarded".to_string();
            persist_task_result(&app_handle, result)?;
        }
    } else if let Ok(mut result) = load_task_result(&app_handle, &task_id) {
        result.status = "discarded".to_string();
        persist_task_result(&app_handle, &result)?;
    }
    Ok(())
}

pub fn smart_culling_apply_task_result(
    task_id: String,
    items: Vec<SmartCullingReviewItem>,
    app_handle: AppHandle,
) -> Result<SmartCullingApplyResult, String> {
    let mut task_result = {
        let tasks = TASKS.lock().map_err(|e| e.to_string())?;
        let result = tasks
            .get(&task_id)
            .and_then(|task| task.result.as_ref())
            .cloned();
        drop(tasks);
        result
            .or_else(|| load_task_result(&app_handle, &task_id).ok())
            .ok_or_else(|| "未找到智能选图任务。".to_string())?
    };

    if task_result.preview_only {
        return Err("当前任务为仅分析不写入模式，不能应用结果。".to_string());
    }

    let mut applied = 0usize;
    let mut skipped = 0usize;
    let mut applied_paths = Vec::new();
    let mut skipped_paths = Vec::new();
    let applied_at = chrono::Local::now().to_rfc3339();
    let settings = load_settings(app_handle.clone()).unwrap_or_default();
    let enable_xmp_sync = settings.enable_xmp_sync.unwrap_or(false);
    let create_xmp_if_missing = settings.create_xmp_if_missing.unwrap_or(false);
    let mut updated_items = items;
    let mut changes: Vec<(SmartCullingAppliedSnapshot, PathBuf, PathBuf)> = Vec::new();

    for item in &mut updated_items {
        if matches!(item.status.as_str(), "skipped" | "failed") {
            skipped += 1;
            skipped_paths.push(item.path.clone());
            continue;
        }

        let (_, sidecar_path) = parse_virtual_path(&item.path);
        let (source_path, _) = parse_virtual_path(&item.path);
        let mut metadata = read_metadata(&sidecar_path);
        let previous_metadata = metadata.clone();

        if metadata.rating > 0 && !is_previous_smart_rating(&metadata, metadata.rating) {
            mark_item_skipped(item, "已存在人工评分，智能选图不会覆盖");
            skipped += 1;
            skipped_paths.push(item.path.clone());
            continue;
        }

        let mut feature_data = metadata.feature_data.take().unwrap_or_else(|| json!({}));
        if !feature_data.is_object() {
            feature_data = json!({});
        }

        let smart_data = json!({
            "schemaVersion": 1,
            "taskId": task_id.clone(),
            "source": "smart_culling",
            "status": item.status.clone(),
            "rating": item.rating,
            "appliedRating": item.rating,
            "confidence": item.confidence,
            "degraded": item.degraded,
            "reasonCodes": item.reason_codes.clone(),
            "reasonText": item.reason_text.clone(),
            "groupId": item.group_id.clone(),
            "groupRank": item.group_rank,
            "groupSize": item.group_size,
            "colorLabel": item.color_label.clone(),
            "createdAt": task_result.created_at.clone(),
            "appliedAt": applied_at.clone(),
        });

        if let Some(obj) = feature_data.as_object_mut() {
            obj.insert("smartCulling".to_string(), smart_data);
        }

        metadata.feature_data = Some(feature_data);
        metadata.rating = item.rating;
        apply_color_label(&mut metadata, item.color_label.as_deref());
        let snapshot = SmartCullingAppliedSnapshot {
            path: item.path.clone(),
            sidecar_path: sidecar_path.to_string_lossy().to_string(),
            previous_metadata,
            applied_metadata: metadata,
        };
        changes.push((snapshot, sidecar_path, source_path));
        applied_paths.push(item.path.clone());
        applied += 1;
    }

    let snapshots: Vec<SmartCullingAppliedSnapshot> = changes
        .iter()
        .map(|(snapshot, _, _)| snapshot.clone())
        .collect();
    task_result.status = "applied".to_string();
    task_result.items = updated_items.clone();
    task_result.applied_at = Some(applied_at.clone());
    task_result.revoked_at = None;
    task_result.error = None;
    task_result.summary = summarize(&task_result.items);
    persist_task_report_pdf(&app_handle, &mut task_result)?;
    persist_apply_snapshots(&app_handle, &task_id, &snapshots)?;

    let mut written_snapshots = Vec::new();
    for (snapshot, sidecar_path, _) in &changes {
        if let Err(error) = write_metadata_atomic(sidecar_path, &snapshot.applied_metadata) {
            rollback_written_snapshots(&written_snapshots);
            task_result.status = "failed".to_string();
            task_result.error = Some(format!("应用结果失败，已回滚已写入项：{}", error));
            let _ = persist_task_report_pdf(&app_handle, &mut task_result);
            let _ = persist_task_result(&app_handle, &task_result);
            return Err(format!("应用智能选图结果失败，已回滚已写入项：{}", error));
        }
        written_snapshots.push(snapshot.clone());
    }

    if let Err(error) = persist_task_result(&app_handle, &task_result) {
        rollback_written_snapshots(&written_snapshots);
        task_result.status = "failed".to_string();
        task_result.error = Some(format!("保存任务结果失败，已回滚照片写入：{}", error));
        let _ = persist_task_report_pdf(&app_handle, &mut task_result);
        let _ = persist_task_result(&app_handle, &task_result);
        return Err(format!(
            "保存智能选图任务结果失败，已回滚照片写入：{}",
            error
        ));
    }

    if enable_xmp_sync {
        for (snapshot, _, source_path) in &changes {
            sync_metadata_to_xmp(
                source_path,
                &snapshot.applied_metadata,
                create_xmp_if_missing,
            );
        }
    }

    let mut tasks = TASKS.lock().map_err(|e| e.to_string())?;
    if let Some(task) = tasks.get_mut(&task_id) {
        task.status = "applied".to_string();
        task.result = Some(task_result.clone());
    }

    Ok(SmartCullingApplyResult {
        task_id,
        applied,
        skipped,
        applied_paths,
        skipped_paths,
        report_path: task_result.report_path,
    })
}

pub fn smart_culling_list_recent_tasks(
    app_handle: AppHandle,
) -> Result<Vec<SmartCullingHistoryItem>, String> {
    let mut results = Vec::new();
    let tasks_dir = smart_culling_tasks_dir(&app_handle)?;
    let entries = fs::read_dir(tasks_dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path().join("task.json");
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };
        let Ok(result) = serde_json::from_str::<SmartCullingTaskResult>(&content) else {
            continue;
        };
        results.push(SmartCullingHistoryItem {
            task_id: result.task_id,
            status: result.status,
            created_at: result.created_at,
            summary: result.summary,
            degraded: result.degraded,
            preview_only: result.preview_only,
            report_path: result.report_path,
        });
    }
    results.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    results.truncate(RECENT_TASK_LIMIT);
    Ok(results)
}

pub fn smart_culling_export_report_pdf(
    params: SmartCullingExportReportParams,
    app_handle: AppHandle,
) -> Result<SmartCullingReportResult, String> {
    let mut result = load_task_result(&app_handle, &params.task_id).or_else(|_| {
        let tasks = TASKS.lock().map_err(|e| e.to_string())?;
        tasks
            .get(&params.task_id)
            .and_then(|task| task.result.clone())
            .ok_or_else(|| "未找到智能选图任务。".to_string())
    })?;
    result.items = params.items;
    result.summary = summarize(&result.items);
    let report_path = task_dir(&app_handle, &result.task_id)?.join("report.pdf");
    write_report_pdf(&result, &report_path)?;
    result.report_path = Some(report_path.to_string_lossy().to_string());
    persist_task_result(&app_handle, &result)?;
    Ok(SmartCullingReportResult {
        task_id: result.task_id,
        report_path: report_path.to_string_lossy().to_string(),
    })
}

pub fn smart_culling_undo_task(
    task_id: String,
    app_handle: AppHandle,
) -> Result<SmartCullingUndoResult, String> {
    ensure_latest_applied_task(&app_handle, &task_id)?;
    let mut result = load_task_result(&app_handle, &task_id)?;
    if result.status != "applied" {
        return Err("只能撤销最近一次已应用的智能选图任务。".to_string());
    }

    let snapshots = load_apply_snapshots(&app_handle, &task_id)?;
    let settings = load_settings(app_handle.clone()).unwrap_or_default();
    let enable_xmp_sync = settings.enable_xmp_sync.unwrap_or(false);
    let create_xmp_if_missing = settings.create_xmp_if_missing.unwrap_or(false);
    let mut restored = 0usize;
    let mut skipped = 0usize;

    for snapshot in &snapshots {
        let sidecar_path = PathBuf::from(&snapshot.sidecar_path);
        let current_metadata = read_metadata(&sidecar_path);
        if !metadata_matches(&current_metadata, &snapshot.applied_metadata) {
            skipped += 1;
            continue;
        }
        write_metadata_atomic(&sidecar_path, &snapshot.previous_metadata)?;
        if enable_xmp_sync {
            let (source_path, _) = parse_virtual_path(&snapshot.path);
            sync_metadata_to_xmp(
                &source_path,
                &snapshot.previous_metadata,
                create_xmp_if_missing,
            );
        }
        restored += 1;
    }

    if restored == 0 && skipped > 0 {
        return Err("未找到可安全撤销的照片；可能已被用户手动修改。".to_string());
    }

    result.status = "revoked".to_string();
    result.revoked_at = Some(chrono::Local::now().to_rfc3339());
    let _ = persist_task_report_pdf(&app_handle, &mut result);
    persist_task_result(&app_handle, &result)?;

    let mut tasks = TASKS.lock().map_err(|e| e.to_string())?;
    if let Some(task) = tasks.get_mut(&task_id) {
        task.status = "revoked".to_string();
        task.result = Some(result);
    }

    Ok(SmartCullingUndoResult {
        task_id,
        restored,
        skipped,
    })
}

pub fn smart_culling_list_presets(
    app_handle: AppHandle,
) -> Result<Vec<SmartCullingUserPreset>, String> {
    load_user_presets(&app_handle)
}

pub fn smart_culling_save_preset(
    params: SmartCullingSavePresetParams,
    app_handle: AppHandle,
) -> Result<SmartCullingUserPreset, String> {
    let name = params.name.trim();
    if name.is_empty() {
        return Err("预设名称不能为空。".to_string());
    }

    let mut presets = load_user_presets(&app_handle)?;
    let now = chrono::Local::now().to_rfc3339();
    let saved = if let Some(id) = params.id.filter(|id| !id.trim().is_empty()) {
        if let Some(existing) = presets.iter_mut().find(|preset| preset.id == id) {
            existing.name = name.to_string();
            existing.updated_at = now.clone();
            existing.config = params.config;
            existing.clone()
        } else {
            let preset = SmartCullingUserPreset {
                id,
                name: name.to_string(),
                created_at: now.clone(),
                updated_at: now.clone(),
                config: params.config,
            };
            presets.push(preset.clone());
            preset
        }
    } else {
        let preset = SmartCullingUserPreset {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            created_at: now.clone(),
            updated_at: now,
            config: params.config,
        };
        presets.push(preset.clone());
        preset
    };

    presets.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    persist_user_presets(&app_handle, &presets)?;
    Ok(saved)
}

pub fn smart_culling_delete_preset(id: String, app_handle: AppHandle) -> Result<(), String> {
    let mut presets = load_user_presets(&app_handle)?;
    let before = presets.len();
    presets.retain(|preset| preset.id != id);
    if presets.len() == before {
        return Err("未找到智能选图预设。".to_string());
    }
    persist_user_presets(&app_handle, &presets)
}

fn run_task(
    task_id: String,
    params: SmartCullingStartParams,
    app_handle: AppHandle,
    cancel: Arc<AtomicBool>,
) -> Result<SmartCullingTaskResult, String> {
    emit_progress(&app_handle, &task_id, 0, params.paths.len(), "准备任务");
    let models_status = smart_culling_check_models(app_handle.clone())?;
    if !models_status.can_run_full && !params.allow_degraded {
        return Err("缺少智能选图模型包。".to_string());
    }
    emit_progress(
        &app_handle,
        &task_id,
        0,
        params.paths.len(),
        "检查智能选图模型",
    );
    let clip_models = match prepare_clip_models_if_available(&app_handle, &models_status) {
        Ok(models) => models,
        Err(error) => {
            if !params.allow_degraded {
                return Err(error);
            }
            let _ = app_handle.emit(
                "smart-culling:model-unavailable",
                json!({ "taskId": task_id.clone(), "error": error }),
            );
            None
        }
    };
    let degraded = clip_models.is_none();
    let assets = normalize_assets(&params);
    let total = assets.len().max(1);

    let skipped_items: Vec<SmartCullingReviewItem> = assets
        .iter()
        .filter_map(|asset| {
            asset
                .skip_reason
                .as_ref()
                .map(|reason| SmartCullingReviewItem {
                    path: asset.display_path.clone(),
                    file_name: asset.file_name.clone(),
                    rating: 0,
                    status: "skipped".to_string(),
                    color_label: None,
                    score: 0.0,
                    confidence: 1.0,
                    degraded,
                    reason_codes: vec!["skipped".to_string()],
                    reason_text: reason.clone(),
                    group_id: None,
                    group_rank: None,
                    group_size: None,
                    skip_reason: Some(reason.clone()),
                })
        })
        .collect();

    let candidates: Vec<&SmartCullingAsset> = assets
        .iter()
        .filter(|asset| asset.skip_reason.is_none())
        .collect();

    let app_settings = load_settings(app_handle.clone()).unwrap_or_default();
    let highlight_compression = app_settings.raw_highlight_compression.unwrap_or(2.5);
    let linear_mode = app_settings.linear_raw_mode;
    let completed = Arc::new(AtomicUsize::new(0));
    let hasher = HasherConfig::new()
        .hash_alg(HashAlg::DoubleGradient)
        .hash_size(16, 16)
        .to_hasher();

    let analyses: Vec<Result<ImageAnalysisData, SmartCullingReviewItem>> = candidates
        .par_iter()
        .map(|asset| {
            if cancel.load(Ordering::SeqCst) {
                return Err(failed_item(asset, "任务已取消"));
            }
            let current = completed.fetch_add(1, Ordering::SeqCst) + 1;
            emit_progress(
                &app_handle,
                &task_id,
                current,
                total,
                "RAW 解码 / 生成分析图",
            );
            analyze_asset(
                asset,
                &hasher,
                highlight_compression,
                linear_mode.clone(),
                &params,
                clip_models.as_ref(),
                degraded,
            )
            .map_err(|error| failed_item(asset, &error))
        })
        .collect();

    if cancel.load(Ordering::SeqCst) {
        return Err("任务已取消。".to_string());
    }

    emit_progress(&app_handle, &task_id, total, total, "相似度分析与分组");

    let mut successful = Vec::new();
    let mut failed_items = Vec::new();
    for analysis in analyses {
        match analysis {
            Ok(data) => successful.push(data),
            Err(item) => failed_items.push(item),
        }
    }

    let mut items = apply_similarity_groups(successful, params.keep_per_group.max(1), degraded);
    items.extend(skipped_items);
    items.extend(failed_items);
    sort_review_items(&mut items);
    let summary = summarize(&items);

    Ok(SmartCullingTaskResult {
        task_id,
        status: "review_ready".to_string(),
        preview_only: params.preview_only,
        degraded,
        created_at: chrono::Local::now().to_rfc3339(),
        applied_at: None,
        revoked_at: None,
        report_path: None,
        error: None,
        summary,
        items,
    })
}

fn normalize_assets(params: &SmartCullingStartParams) -> Vec<SmartCullingAsset> {
    let mut by_key: HashMap<String, Vec<(String, PathBuf, PathBuf)>> = HashMap::new();
    let mut ordered_keys = Vec::new();

    for display_path in &params.paths {
        let (source_path, sidecar_path) = parse_virtual_path(display_path);
        let key = asset_key(&source_path);
        if !by_key.contains_key(&key) {
            ordered_keys.push(key.clone());
        }
        by_key
            .entry(key)
            .or_default()
            .push((display_path.clone(), source_path, sidecar_path));
    }

    let mut assets = Vec::new();
    for key in ordered_keys {
        let Some(entries) = by_key.remove(&key) else {
            continue;
        };

        let raw_entry = entries
            .iter()
            .find(|(_, source_path, _)| is_raw_file(source_path))
            .cloned();
        let selected = raw_entry.or_else(|| entries.first().cloned());
        let Some((display_path, source_path, sidecar_path)) = selected else {
            continue;
        };

        let file_name = source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string();

        let metadata = read_metadata(&sidecar_path);
        let mut skip_reason = None;

        if display_path.contains("?vc=") {
            skip_reason = Some("虚拟副本不单独参与智能选图".to_string());
        } else if !is_supported_image_file(&source_path) {
            skip_reason = Some("不支持的图片格式".to_string());
        } else if !is_raw_file(&source_path) {
            skip_reason = Some("未找到同名 RAW，智能选图 V1 仅分析 RAW 原图".to_string());
        } else if metadata.rating > 0 && !is_previous_smart_rating(&metadata, metadata.rating) {
            skip_reason = Some("已存在人工评分，智能选图不会覆盖".to_string());
        } else if !params.include_edited && is_user_edited(&metadata.adjustments) {
            skip_reason = Some("已存在修图调整，智能选图默认跳过".to_string());
        }

        assets.push(SmartCullingAsset {
            display_path,
            source_path,
            file_name,
            skip_reason,
        });
    }

    assets
}

fn analyze_asset(
    asset: &SmartCullingAsset,
    hasher: &image_hasher::Hasher,
    highlight_compression: f32,
    linear_mode: String,
    params: &SmartCullingStartParams,
    clip_models: Option<&Arc<crate::ai_processing::ClipModels>>,
    degraded: bool,
) -> Result<ImageAnalysisData, String> {
    let bytes = fs::read(&asset.source_path).map_err(|e| e.to_string())?;
    let path_str = asset.source_path.to_string_lossy().to_string();
    let img = image_loader::load_base_image_from_bytes(
        &bytes,
        &path_str,
        true,
        highlight_compression,
        linear_mode,
        None,
    )
    .map_err(|e| e.to_string())?;

    let (width, height) = img.dimensions();
    let thumbnail = img.thumbnail(720, 720);
    let gray = thumbnail.to_luma8();
    let sharpness = calculate_laplacian_variance(&gray);
    let exposure = calculate_exposure_metric(&gray);
    let center_focus = calculate_center_focus_metric(&gray);
    let base_score = score_image(sharpness, center_focus, exposure, params);
    let aesthetic_assist = score_aesthetic_preference(&thumbnail, params);
    let clip_assist = run_clip_quality_assist(clip_models, &thumbnail, params, base_score);
    let score =
        (base_score + aesthetic_assist.score_delta + clip_assist.score_delta).clamp(0.0, 1.0);
    let (rating, status) = rating_from_score(score, &params.preset);
    let (mut reason_codes, mut reason_text) = build_reasons(
        sharpness,
        center_focus,
        exposure,
        degraded,
        params,
        width,
        height,
    );
    merge_aesthetic_reasons(&mut reason_codes, &mut reason_text, &aesthetic_assist);
    merge_clip_reasons(&mut reason_codes, &mut reason_text, &clip_assist);
    let hash = hasher.hash_image(&thumbnail);

    Ok(ImageAnalysisData {
        hash,
        item: SmartCullingReviewItem {
            path: asset.display_path.clone(),
            file_name: asset.file_name.clone(),
            rating,
            status,
            color_label: None,
            score,
            confidence: confidence_from_analysis(degraded, &clip_assist),
            degraded,
            reason_codes,
            reason_text,
            group_id: None,
            group_rank: None,
            group_size: None,
            skip_reason: None,
        },
    })
}

fn apply_similarity_groups(
    analyses: Vec<ImageAnalysisData>,
    keep_per_group: usize,
    degraded: bool,
) -> Vec<SmartCullingReviewItem> {
    let mut processed = vec![false; analyses.len()];
    let mut items = Vec::new();

    for i in 0..analyses.len() {
        if processed[i] {
            continue;
        }
        processed[i] = true;
        let mut group_indices = vec![i];
        let mut queue = VecDeque::from([i]);

        while let Some(current) = queue.pop_front() {
            for j in 0..analyses.len() {
                if processed[j] {
                    continue;
                }
                if analyses[current].hash.dist(&analyses[j].hash) <= 28 {
                    processed[j] = true;
                    group_indices.push(j);
                    queue.push_back(j);
                }
            }
        }

        group_indices.sort_by(|&a, &b| {
            analyses[b]
                .item
                .score
                .partial_cmp(&analyses[a].item.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if group_indices.len() > 1 {
            let group_id = Uuid::new_v4().to_string();
            let group_size = group_indices.len();
            for (rank_index, idx) in group_indices.into_iter().enumerate() {
                let mut item = analyses[idx].item.clone();
                let rank = rank_index + 1;
                item.group_id = Some(group_id.clone());
                item.group_rank = Some(rank);
                item.group_size = Some(group_size);
                item.reason_codes.push("重复".to_string());
                if rank > keep_per_group {
                    item.rating = item.rating.min(2);
                    item.status = "review".to_string();
                    item.reason_text = format!("相似组第 {} 名，建议折叠复核", rank);
                } else {
                    item.reason_text = format!("相似组第 {} 名，建议优先保留", rank);
                }
                item.degraded = item.degraded || degraded;
                items.push(item);
            }
        } else {
            items.push(analyses[i].item.clone());
        }
    }

    items
}

#[derive(Default)]
struct ClipAssistResult {
    used: bool,
    tags: Vec<String>,
    score_delta: f64,
    error: Option<String>,
}

#[derive(Default)]
struct AestheticAssistResult {
    score_delta: f64,
    reason_code: Option<String>,
    reason_text: Option<String>,
}

fn prepare_clip_models_if_available(
    app_handle: &AppHandle,
    models_status: &SmartCullingModelsStatus,
) -> Result<Option<Arc<crate::ai_processing::ClipModels>>, String> {
    if !models_status.can_run_full {
        return Ok(None);
    }

    let app_state = app_handle.state::<crate::AppState>();
    tauri::async_runtime::block_on(crate::ai_processing::get_or_init_clip_models(
        app_handle,
        &app_state.ai_state,
        &app_state.ai_init_lock,
    ))
    .map(Some)
    .map_err(|error| format!("智能选图模型初始化失败：{}", error))
}

fn run_clip_quality_assist(
    clip_models: Option<&Arc<crate::ai_processing::ClipModels>>,
    image: &DynamicImage,
    params: &SmartCullingStartParams,
    base_score: f64,
) -> ClipAssistResult {
    let Some(clip_models) = clip_models else {
        return ClipAssistResult::default();
    };

    if !should_run_clip_assist(base_score) {
        return ClipAssistResult {
            used: true,
            ..ClipAssistResult::default()
        };
    }

    match crate::tagging::generate_tags_with_clip(
        image,
        &clip_models.model,
        &clip_models.tokenizer,
        Some(clip_prompt_candidates(params)),
        4,
    ) {
        Ok(tags) => {
            let score_delta = score_clip_tags(&tags);
            ClipAssistResult {
                used: true,
                tags,
                score_delta,
                error: None,
            }
        }
        Err(error) => ClipAssistResult {
            used: true,
            tags: Vec::new(),
            score_delta: 0.0,
            error: Some(error.to_string()),
        },
    }
}

fn should_run_clip_assist(base_score: f64) -> bool {
    (0.24..=0.88).contains(&base_score)
}

fn clip_prompt_candidates(params: &SmartCullingStartParams) -> Vec<String> {
    let mut prompts = vec![
        "a sharp professional photograph".to_string(),
        "a blurry photograph".to_string(),
        "a well exposed photograph".to_string(),
        "an underexposed photograph".to_string(),
        "an overexposed photograph".to_string(),
        "a beautiful high quality photograph".to_string(),
        "a low quality photograph".to_string(),
        "a photo with strong composition".to_string(),
        "a photo with poor composition".to_string(),
    ];

    match params.aesthetic_preference.as_str() {
        "dark_tone" => prompts.extend([
            "a moody dark tone photograph".to_string(),
            "a flat bright photograph".to_string(),
        ]),
        "film" => prompts.extend([
            "a film look photograph".to_string(),
            "a sterile digital photograph".to_string(),
        ]),
        "shallow_depth" => prompts.extend([
            "a photograph with shallow depth of field".to_string(),
            "a photograph with distracting background".to_string(),
        ]),
        "candid_emotion" => prompts.extend([
            "a candid emotional photograph".to_string(),
            "a stiff posed photograph".to_string(),
        ]),
        _ => {}
    }

    match params.mode.as_str() {
        "portrait" | "wedding_event" | "family_children" => {
            prompts.extend([
                "a portrait photograph with a clear face".to_string(),
                "a portrait photograph with a blurry face".to_string(),
                "a smiling portrait photograph".to_string(),
            ]);
        }
        "landscape" => {
            prompts.extend([
                "a beautiful landscape photograph".to_string(),
                "a landscape photograph with dramatic light".to_string(),
            ]);
        }
        "sports_wildlife" => {
            prompts.extend([
                "a sharp action photograph".to_string(),
                "a blurry action photograph".to_string(),
            ]);
        }
        "product_still" => {
            prompts.extend([
                "a clean product photograph".to_string(),
                "a poorly lit product photograph".to_string(),
            ]);
        }
        _ => {}
    }

    if params.face_analysis_enabled {
        let checks: std::collections::HashSet<&str> =
            params.face_checks.iter().map(String::as_str).collect();
        if checks.is_empty() || checks.contains("closed_eyes") {
            prompts.extend([
                "a portrait with open eyes".to_string(),
                "a portrait with closed eyes".to_string(),
            ]);
        }
        if checks.is_empty() || checks.contains("blurred_face") {
            prompts.extend([
                "a portrait with a sharp face".to_string(),
                "a portrait with a blurry face".to_string(),
            ]);
        }
        if checks.is_empty() || checks.contains("abnormal_expression") {
            prompts.extend([
                "a portrait with natural expression".to_string(),
                "a portrait with awkward expression".to_string(),
            ]);
        }
        if checks.is_empty() || checks.contains("smile") {
            prompts.push("a smiling portrait photograph".to_string());
        }
        if checks.is_empty() || checks.contains("best_group_expression") {
            prompts.extend([
                "a group photo with good expressions".to_string(),
                "a group photo with bad expressions".to_string(),
            ]);
        }
        if checks.is_empty() || checks.contains("looking_camera") {
            prompts.extend([
                "a subject looking at the camera".to_string(),
                "a subject looking away from the camera".to_string(),
            ]);
        }
    }

    prompts
}

fn score_clip_tags(tags: &[String]) -> f64 {
    let mut delta: f64 = 0.0;
    for tag in tags {
        let normalized = tag.to_lowercase();
        if normalized.contains("sharp")
            || normalized.contains("well exposed")
            || normalized.contains("beautiful")
            || normalized.contains("strong composition")
            || normalized.contains("clear face")
            || normalized.contains("open eyes")
            || normalized.contains("sharp face")
            || normalized.contains("natural expression")
            || normalized.contains("smiling")
            || normalized.contains("good expressions")
            || normalized.contains("looking at the camera")
            || normalized.contains("moody dark tone")
            || normalized.contains("film look")
            || normalized.contains("shallow depth")
            || normalized.contains("candid emotional")
            || normalized.contains("dramatic light")
            || normalized.contains("clean product")
        {
            delta += 0.025;
        }
        if normalized.contains("blurry")
            || normalized.contains("underexposed")
            || normalized.contains("overexposed")
            || normalized.contains("low quality")
            || normalized.contains("poor composition")
            || normalized.contains("closed eyes")
            || normalized.contains("blurry face")
            || normalized.contains("awkward expression")
            || normalized.contains("bad expressions")
            || normalized.contains("looking away")
            || normalized.contains("flat bright")
            || normalized.contains("sterile digital")
            || normalized.contains("distracting background")
            || normalized.contains("stiff posed")
            || normalized.contains("poorly lit")
        {
            delta -= 0.04;
        }
    }
    delta.clamp(-0.09, 0.08)
}

fn merge_clip_reasons(
    reason_codes: &mut Vec<String>,
    reason_text: &mut String,
    clip_assist: &ClipAssistResult,
) {
    if !clip_assist.used {
        return;
    }

    if let Some(error) = &clip_assist.error {
        reason_codes.push("模型辅助失败".to_string());
        append_reason(
            reason_text,
            &format!("模型辅助失败，已降级基础指标：{}", error),
        );
    } else if clip_assist.score_delta > 0.01 {
        reason_codes.push("模型辅助加分".to_string());
        append_reason(reason_text, "模型辅助判定画面质量较好");
    } else if clip_assist.score_delta < -0.01 {
        reason_codes.push("模型辅助扣分".to_string());
        append_reason(reason_text, "模型辅助发现画面质量风险");
    } else if !clip_assist.tags.is_empty() {
        reason_codes.push("模型辅助".to_string());
    }

    reason_codes.sort();
    reason_codes.dedup();
}

fn score_aesthetic_preference(
    image: &DynamicImage,
    params: &SmartCullingStartParams,
) -> AestheticAssistResult {
    let rgb = image.to_rgb8();
    let mut luminance_sum = 0.0;
    let mut saturation_sum = 0.0;
    let mut count = 0.0;
    for pixel in rgb.pixels().step_by(12) {
        let r = pixel[0] as f64 / 255.0;
        let g = pixel[1] as f64 / 255.0;
        let b = pixel[2] as f64 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        luminance_sum += (0.2126 * r) + (0.7152 * g) + (0.0722 * b);
        saturation_sum += if max <= f64::EPSILON {
            0.0
        } else {
            (max - min) / max
        };
        count += 1.0;
    }

    if count <= 0.0 {
        return AestheticAssistResult::default();
    }

    let luminance = luminance_sum / count;
    let saturation = saturation_sum / count;
    match params.aesthetic_preference.as_str() {
        "dark_tone" if luminance < 0.42 && saturation > 0.12 => AestheticAssistResult {
            score_delta: 0.035,
            reason_code: Some("暗调偏好".to_string()),
            reason_text: Some("符合暗调偏好".to_string()),
        },
        "dark_tone" if luminance > 0.72 => AestheticAssistResult {
            score_delta: -0.025,
            reason_code: Some("暗调偏好不匹配".to_string()),
            reason_text: Some("画面偏亮，不符合暗调偏好".to_string()),
        },
        "film" if saturation < 0.38 && luminance > 0.18 && luminance < 0.78 => {
            AestheticAssistResult {
                score_delta: 0.025,
                reason_code: Some("胶片偏好".to_string()),
                reason_text: Some("色彩克制，接近胶片偏好".to_string()),
            }
        }
        "shallow_depth" => AestheticAssistResult {
            score_delta: 0.015,
            reason_code: Some("浅景深偏好".to_string()),
            reason_text: Some("已按浅景深偏好提高主体清晰权重".to_string()),
        },
        "candid_emotion" => AestheticAssistResult {
            score_delta: 0.012,
            reason_code: Some("抓拍情绪偏好".to_string()),
            reason_text: Some("已按抓拍情绪偏好保留更多边界候选".to_string()),
        },
        _ => AestheticAssistResult::default(),
    }
}

fn merge_aesthetic_reasons(
    reason_codes: &mut Vec<String>,
    reason_text: &mut String,
    aesthetic_assist: &AestheticAssistResult,
) {
    if let Some(code) = &aesthetic_assist.reason_code {
        reason_codes.push(code.clone());
    }
    if let Some(text) = &aesthetic_assist.reason_text {
        append_reason(reason_text, text);
    }
    reason_codes.sort();
    reason_codes.dedup();
}

fn append_reason(reason_text: &mut String, part: &str) {
    if reason_text.is_empty() {
        reason_text.push_str(part);
    } else {
        reason_text.push('，');
        reason_text.push_str(part);
    }
}

fn confidence_from_analysis(degraded: bool, clip_assist: &ClipAssistResult) -> f64 {
    if clip_assist.error.is_some() {
        0.66
    } else if degraded {
        0.72
    } else if clip_assist.used {
        0.88
    } else {
        0.82
    }
}

fn score_image(
    sharpness: f64,
    center_focus: f64,
    exposure: f64,
    params: &SmartCullingStartParams,
) -> f64 {
    let normalized_sharpness = ((sharpness + 1.0).log10() / 3.5).clamp(0.0, 1.0);
    let normalized_center = ((center_focus + 1.0).log10() / 3.5).clamp(0.0, 1.0);

    let (sharp_w, center_w, exposure_w) = match params.mode.as_str() {
        "portrait" | "wedding_event" | "family_children" => (0.35, 0.45, 0.20),
        "landscape" | "architecture" => (0.48, 0.18, 0.34),
        "product_still" => (0.40, 0.28, 0.32),
        "sports_wildlife" => (0.50, 0.35, 0.15),
        _ => (0.42, 0.30, 0.28),
    };

    (normalized_sharpness * sharp_w) + (normalized_center * center_w) + (exposure * exposure_w)
}

fn rating_from_score(score: f64, preset: &str) -> (u8, String) {
    let offset = match preset {
        "strict" => 0.06,
        "loose" => -0.06,
        _ => 0.0,
    };
    let s = score - offset;
    if s >= 0.82 {
        (5, "selected".to_string())
    } else if s >= 0.68 {
        (4, "selected".to_string())
    } else if s >= 0.50 {
        (3, "review".to_string())
    } else if s >= 0.32 {
        (2, "review".to_string())
    } else {
        (1, "reject_suggestion".to_string())
    }
}

fn build_reasons(
    sharpness: f64,
    center_focus: f64,
    exposure: f64,
    degraded: bool,
    params: &SmartCullingStartParams,
    width: u32,
    height: u32,
) -> (Vec<String>, String) {
    let mut codes = Vec::new();
    let mut parts = Vec::new();

    if sharpness > 260.0 {
        codes.push("主体清晰".to_string());
        parts.push("整体清晰度较好".to_string());
    } else if sharpness < 80.0 {
        codes.push("主体不清晰".to_string());
        parts.push("整体清晰度偏低".to_string());
    }

    if center_focus > sharpness * 0.8 {
        codes.push("中心清晰".to_string());
        parts.push("中心区域可用".to_string());
    }

    if exposure < 0.62 {
        codes.push("曝光异常".to_string());
        parts.push("存在明显过暗或过曝风险".to_string());
    } else {
        codes.push("曝光正常".to_string());
        parts.push("曝光分布正常".to_string());
    }

    if params.face_analysis_enabled {
        codes.push("人像表情辅助".to_string());
        parts.push("已启用人像表情辅助判定".to_string());
    }

    if degraded {
        codes.push("可信度降低".to_string());
    }

    if width > 0 && height > 0 {
        codes.push("RAW分析".to_string());
    }

    codes.sort();
    codes.dedup();

    if parts.is_empty() {
        parts.push("基础画质指标稳定".to_string());
    }

    (codes, parts.join("，"))
}

fn calculate_laplacian_variance(image: &GrayImage) -> f64 {
    let (width, height) = image.dimensions();
    if width < 3 || height < 3 {
        return 0.0;
    }

    let mut values = Vec::with_capacity(((width - 2) * (height - 2)) as usize);
    let mut sum = 0.0;
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let center = image.get_pixel(x, y)[0] as i32;
            let north = image.get_pixel(x, y - 1)[0] as i32;
            let south = image.get_pixel(x, y + 1)[0] as i32;
            let west = image.get_pixel(x - 1, y)[0] as i32;
            let east = image.get_pixel(x + 1, y)[0] as i32;
            let value = (north + south + west + east - 4 * center) as f64;
            values.push(value);
            sum += value;
        }
    }

    let mean = sum / values.len() as f64;
    values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / values.len() as f64
}

fn calculate_center_focus_metric(image: &GrayImage) -> f64 {
    let (width, height) = image.dimensions();
    if width < 4 || height < 4 {
        return calculate_laplacian_variance(image);
    }
    let crop = imageops::crop_imm(image, width / 4, height / 4, width / 2, height / 2).to_image();
    calculate_laplacian_variance(&crop)
}

fn calculate_exposure_metric(image: &GrayImage) -> f64 {
    let histogram = imageproc::stats::histogram(image);
    let total = (image.width() * image.height()) as f64;
    if total == 0.0 {
        return 0.0;
    }
    let dark = histogram.channels[0][0..5].iter().sum::<u32>() as f64 / total;
    let bright = histogram.channels[0][250..256].iter().sum::<u32>() as f64 / total;
    (1.0 - ((dark + bright) * 4.5)).clamp(0.0, 1.0)
}

fn summarize(items: &[SmartCullingReviewItem]) -> SmartCullingSummary {
    let mut summary = SmartCullingSummary::default();
    for item in items {
        match item.status.as_str() {
            "selected" => {
                summary.analyzed += 1;
                summary.selected += 1;
            }
            "review" => {
                summary.analyzed += 1;
                summary.review += 1;
            }
            "reject_suggestion" => {
                summary.analyzed += 1;
                summary.reject_suggestion += 1;
            }
            "skipped" => summary.skipped += 1,
            "failed" => summary.failed += 1,
            _ => {}
        }
    }
    summary
}

fn sort_review_items(items: &mut [SmartCullingReviewItem]) {
    fn order(status: &str) -> u8 {
        match status {
            "selected" => 0,
            "review" => 1,
            "reject_suggestion" => 2,
            "skipped" => 3,
            "failed" => 4,
            _ => 5,
        }
    }
    items.sort_by(|a, b| {
        order(&a.status)
            .cmp(&order(&b.status))
            .then_with(|| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.file_name.cmp(&b.file_name))
    });
}

fn failed_item(asset: &SmartCullingAsset, reason: &str) -> SmartCullingReviewItem {
    SmartCullingReviewItem {
        path: asset.display_path.clone(),
        file_name: asset.file_name.clone(),
        rating: 0,
        status: "failed".to_string(),
        color_label: None,
        score: 0.0,
        confidence: 0.0,
        degraded: true,
        reason_codes: vec!["失败".to_string()],
        reason_text: reason.to_string(),
        group_id: None,
        group_rank: None,
        group_size: None,
        skip_reason: Some(reason.to_string()),
    }
}

fn build_failed_task_result(
    task_id: String,
    params: &SmartCullingStartParams,
    error: &str,
) -> SmartCullingTaskResult {
    let items: Vec<SmartCullingReviewItem> = params
        .paths
        .iter()
        .map(|path| {
            let (source_path, _) = parse_virtual_path(path);
            let file_name = source_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path)
                .to_string();
            SmartCullingReviewItem {
                path: path.clone(),
                file_name,
                rating: 0,
                status: "failed".to_string(),
                color_label: None,
                score: 0.0,
                confidence: 0.0,
                degraded: true,
                reason_codes: vec!["任务失败".to_string()],
                reason_text: error.to_string(),
                group_id: None,
                group_rank: None,
                group_size: None,
                skip_reason: Some(error.to_string()),
            }
        })
        .collect();
    let items = if items.is_empty() {
        vec![SmartCullingReviewItem {
            path: String::new(),
            file_name: "任务整体失败".to_string(),
            rating: 0,
            status: "failed".to_string(),
            color_label: None,
            score: 0.0,
            confidence: 0.0,
            degraded: true,
            reason_codes: vec!["任务失败".to_string()],
            reason_text: error.to_string(),
            group_id: None,
            group_rank: None,
            group_size: None,
            skip_reason: Some(error.to_string()),
        }]
    } else {
        items
    };

    SmartCullingTaskResult {
        task_id,
        status: "failed".to_string(),
        preview_only: params.preview_only,
        degraded: true,
        created_at: chrono::Local::now().to_rfc3339(),
        applied_at: None,
        revoked_at: None,
        report_path: None,
        error: Some(error.to_string()),
        summary: summarize(&items),
        items,
    }
}

fn mark_item_skipped(item: &mut SmartCullingReviewItem, reason: &str) {
    item.rating = 0;
    item.status = "skipped".to_string();
    item.color_label = None;
    item.score = 0.0;
    item.confidence = 1.0;
    item.reason_codes = vec!["跳过".to_string(), "人工评分保护".to_string()];
    item.reason_text = reason.to_string();
    item.skip_reason = Some(reason.to_string());
}

fn asset_key(path: &Path) -> String {
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_lowercase();
    format!("{}::{}", parent.to_string_lossy(), stem)
}

fn read_metadata(sidecar_path: &Path) -> ImageMetadata {
    fs::read_to_string(sidecar_path)
        .ok()
        .and_then(|content| serde_json::from_str::<ImageMetadata>(&content).ok())
        .unwrap_or_default()
}

fn write_metadata_atomic(sidecar_path: &Path, metadata: &ImageMetadata) -> Result<(), String> {
    if let Some(parent) = sidecar_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp_path = temporary_sidecar_path(sidecar_path, "tmp");
    let json = serde_json::to_string_pretty(metadata).map_err(|e| e.to_string())?;
    {
        let mut file = fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
        file.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
    }
    replace_file_preserving_backup(&tmp_path, sidecar_path)
}

fn temporary_sidecar_path(sidecar_path: &Path, suffix: &str) -> PathBuf {
    let file_name = sidecar_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("metadata.rrdata");
    sidecar_path.with_file_name(format!("{}.{}.{}", file_name, Uuid::new_v4(), suffix))
}

#[cfg(not(target_os = "windows"))]
fn replace_file_preserving_backup(tmp_path: &Path, sidecar_path: &Path) -> Result<(), String> {
    fs::rename(tmp_path, sidecar_path).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
fn replace_file_preserving_backup(tmp_path: &Path, sidecar_path: &Path) -> Result<(), String> {
    if !sidecar_path.exists() {
        return fs::rename(tmp_path, sidecar_path).map_err(|e| e.to_string());
    }

    let backup_path = temporary_sidecar_path(sidecar_path, "bak");
    fs::rename(sidecar_path, &backup_path).map_err(|e| e.to_string())?;
    match fs::rename(tmp_path, sidecar_path) {
        Ok(()) => {
            let _ = fs::remove_file(backup_path);
            Ok(())
        }
        Err(error) => {
            let _ = fs::rename(&backup_path, sidecar_path);
            Err(error.to_string())
        }
    }
}

fn rollback_written_snapshots(snapshots: &[SmartCullingAppliedSnapshot]) {
    for snapshot in snapshots.iter().rev() {
        let sidecar_path = PathBuf::from(&snapshot.sidecar_path);
        if let Err(error) = write_metadata_atomic(&sidecar_path, &snapshot.previous_metadata) {
            eprintln!(
                "Failed to rollback smart culling sidecar {}: {}",
                snapshot.sidecar_path, error
            );
        }
    }
}

fn metadata_matches(a: &ImageMetadata, b: &ImageMetadata) -> bool {
    serde_json::to_value(a).ok() == serde_json::to_value(b).ok()
}

fn is_user_edited(adjustments: &Value) -> bool {
    adjustments.as_object().is_some_and(|object| {
        object.keys().len() > 1 || (object.keys().len() == 1 && !object.contains_key("rating"))
    })
}

fn is_previous_smart_rating(metadata: &ImageMetadata, rating: u8) -> bool {
    metadata
        .feature_data
        .as_ref()
        .and_then(|value| value.get("smartCulling"))
        .and_then(|value| value.get("appliedRating"))
        .and_then(Value::as_u64)
        .is_some_and(|smart_rating| smart_rating as u8 == rating)
}

fn apply_color_label(metadata: &mut ImageMetadata, color_label: Option<&str>) {
    let Some(color_label) = color_label else {
        return;
    };

    let mut tags = metadata.tags.take().unwrap_or_default();
    tags.retain(|tag| !tag.starts_with(COLOR_TAG_PREFIX));
    if color_label != "none" && !color_label.is_empty() {
        tags.push(format!("{}{}", COLOR_TAG_PREFIX, color_label));
    }
    metadata.tags = if tags.is_empty() { None } else { Some(tags) };
}

fn smart_culling_root_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("smart-culling");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn smart_culling_tasks_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let dir = smart_culling_root_dir(app_handle)?.join("tasks");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn smart_culling_presets_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    Ok(smart_culling_root_dir(app_handle)?.join("presets.json"))
}

fn load_user_presets(app_handle: &AppHandle) -> Result<Vec<SmartCullingUserPreset>, String> {
    let path = smart_culling_presets_path(app_handle)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str::<Vec<SmartCullingUserPreset>>(&content).map_err(|e| e.to_string())
}

fn persist_user_presets(
    app_handle: &AppHandle,
    presets: &[SmartCullingUserPreset],
) -> Result<(), String> {
    let path = smart_culling_presets_path(app_handle)?;
    let content = serde_json::to_string_pretty(presets).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

fn task_dir(app_handle: &AppHandle, task_id: &str) -> Result<PathBuf, String> {
    let dir = smart_culling_tasks_dir(app_handle)?.join(task_id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn persist_task_result(
    app_handle: &AppHandle,
    result: &SmartCullingTaskResult,
) -> Result<(), String> {
    let path = task_dir(app_handle, &result.task_id)?.join("task.json");
    let content = serde_json::to_string_pretty(result).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

fn persist_task_report_pdf(
    app_handle: &AppHandle,
    result: &mut SmartCullingTaskResult,
) -> Result<(), String> {
    let report_path = task_dir(app_handle, &result.task_id)?.join("report.pdf");
    write_report_pdf(result, &report_path)?;
    result.report_path = Some(report_path.to_string_lossy().to_string());
    Ok(())
}

fn load_task_result(
    app_handle: &AppHandle,
    task_id: &str,
) -> Result<SmartCullingTaskResult, String> {
    let path = task_dir(app_handle, task_id)?.join("task.json");
    let content = fs::read_to_string(path).map_err(|_| "未找到智能选图任务。".to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn persist_apply_snapshots(
    app_handle: &AppHandle,
    task_id: &str,
    snapshots: &[SmartCullingAppliedSnapshot],
) -> Result<(), String> {
    let path = task_dir(app_handle, task_id)?.join("applied_snapshots.json");
    let content = serde_json::to_string_pretty(snapshots).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

fn load_apply_snapshots(
    app_handle: &AppHandle,
    task_id: &str,
) -> Result<Vec<SmartCullingAppliedSnapshot>, String> {
    let path = task_dir(app_handle, task_id)?.join("applied_snapshots.json");
    let content =
        fs::read_to_string(path).map_err(|_| "未找到可撤销的智能选图写入记录。".to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn prune_recent_tasks(app_handle: &AppHandle) -> Result<(), String> {
    let tasks_dir = smart_culling_tasks_dir(app_handle)?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(&tasks_dir)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let task_path = entry.path().join("task.json");
        let Ok(content) = fs::read_to_string(task_path) else {
            continue;
        };
        let Ok(result) = serde_json::from_str::<SmartCullingTaskResult>(&content) else {
            continue;
        };
        entries.push((result.created_at, entry.path()));
    }

    entries.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in entries.into_iter().skip(RECENT_TASK_LIMIT) {
        let _ = fs::remove_dir_all(path);
    }
    Ok(())
}

fn ensure_latest_applied_task(app_handle: &AppHandle, task_id: &str) -> Result<(), String> {
    let tasks_dir = smart_culling_tasks_dir(app_handle)?;
    let mut applied = Vec::new();
    for entry in fs::read_dir(tasks_dir)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let task_path = entry.path().join("task.json");
        let Ok(content) = fs::read_to_string(task_path) else {
            continue;
        };
        let Ok(result) = serde_json::from_str::<SmartCullingTaskResult>(&content) else {
            continue;
        };
        if result.status == "applied" {
            let sort_key = result
                .applied_at
                .clone()
                .unwrap_or(result.created_at.clone());
            applied.push((sort_key, result.task_id));
        }
    }

    applied.sort_by(|a, b| b.0.cmp(&a.0));
    let Some((_, latest_task_id)) = applied.first() else {
        return Err("没有可撤销的智能选图任务。".to_string());
    };
    if latest_task_id != task_id {
        return Err("只能撤销最近一次已应用的智能选图任务。".to_string());
    }
    Ok(())
}

fn shared_models_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("models");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

async fn download_shared_clip_from_mirror(app_handle: &AppHandle) -> Result<(), String> {
    let models_dir = shared_models_dir(app_handle)?;
    download_model_from_mirror(
        app_handle,
        MIRROR_CLIP_MODEL_URL,
        &models_dir.join(SHARED_CLIP_MODEL_FILENAME),
        SHARED_CLIP_MODEL_SHA256,
        "CLIP Model",
    )
    .await?;
    download_model_from_mirror(
        app_handle,
        MIRROR_CLIP_TOKENIZER_URL,
        &models_dir.join(SHARED_CLIP_TOKENIZER_FILENAME),
        SHARED_CLIP_TOKENIZER_SHA256,
        "CLIP Tokenizer",
    )
    .await
}

async fn download_model_from_mirror(
    app_handle: &AppHandle,
    url: &str,
    destination: &Path,
    expected_hash: &str,
    model_name: &str,
) -> Result<(), String> {
    if verify_sha256(destination, expected_hash)? {
        return Ok(());
    }

    if destination.exists() {
        fs::remove_file(destination).map_err(|e| e.to_string())?;
    }

    let _ = app_handle.emit(
        "smart-culling:model-download-start",
        json!({ "stage": format!("下载 {}", model_name) }),
    );

    let response = reqwest::get(url).await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "{} 下载失败：HTTP {}",
            model_name,
            response.status()
        ));
    }

    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    let tmp_path = destination.with_extension("download");
    fs::write(&tmp_path, &bytes).map_err(|e| e.to_string())?;

    if !verify_sha256(&tmp_path, expected_hash)? {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("{} 校验失败", model_name));
    }

    fs::rename(tmp_path, destination).map_err(|e| e.to_string())
}

fn verify_sha256(path: &Path, expected_hash: &str) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }

    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    let actual = format!("{:x}", hasher.finalize());
    Ok(actual == expected_hash)
}

fn manifest_is_valid(path: &Path) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(manifest) = serde_json::from_str::<Value>(&content) else {
        return false;
    };
    let Some(models) = manifest.get("models").and_then(Value::as_array) else {
        return false;
    };

    let encoder_valid = models.iter().any(|model| {
        model.get("role").and_then(Value::as_str) == Some("image_encoder")
            && model.get("required").and_then(Value::as_bool) == Some(true)
            && model.get("inputSize").and_then(Value::as_u64).is_some()
            && model.get("sha256").and_then(Value::as_str).is_some()
            && model.get("sizeBytes").and_then(Value::as_u64).is_some()
    });
    let tokenizer_valid = models.iter().any(|model| {
        model.get("role").and_then(Value::as_str) == Some("tokenizer")
            && model.get("required").and_then(Value::as_bool) == Some(true)
            && model.get("sha256").and_then(Value::as_str).is_some()
            && model.get("sizeBytes").and_then(Value::as_u64).is_some()
    });

    encoder_valid && tokenizer_valid
}

fn smart_culling_models_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let dir = shared_models_dir(app_handle)?.join("smart-culling");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn write_shared_clip_manifest(app_handle: &AppHandle) -> Result<(), String> {
    let models_dir = smart_culling_models_dir(app_handle)?;
    let shared_dir = shared_models_dir(app_handle)?;
    let manifest_path = models_dir.join("manifest.json");
    let clip_size = fs::metadata(shared_dir.join(SHARED_CLIP_MODEL_FILENAME))
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let tokenizer_size = fs::metadata(shared_dir.join(SHARED_CLIP_TOKENIZER_FILENAME))
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let manifest = json!({
        "schemaVersion": 1,
        "packageName": "RapidRAW Shared CLIP",
        "packageVersion": "2026.05",
        "managedBy": "smart_culling",
        "models": [
            {
                "role": "image_encoder",
                "file": format!("../{}", SHARED_CLIP_MODEL_FILENAME),
                "required": true,
                "inputSize": 224,
                "embeddingDim": 512,
                "sizeBytes": clip_size,
                "sha256": SHARED_CLIP_MODEL_SHA256,
                "source": "CyberTimon/RapidRAW-Models"
            },
            {
                "role": "tokenizer",
                "file": format!("../{}", SHARED_CLIP_TOKENIZER_FILENAME),
                "required": true,
                "sizeBytes": tokenizer_size,
                "sha256": SHARED_CLIP_TOKENIZER_SHA256,
                "source": "CyberTimon/RapidRAW-Models"
            }
        ],
        "notes": "智能选图复用上游 RapidRAW AI Tagging 的 CLIP ONNX 模型，避免重复下载与模型分叉。"
    });
    let content = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    fs::write(manifest_path, content).map_err(|e| e.to_string())
}

fn write_report_pdf(result: &SmartCullingTaskResult, report_path: &Path) -> Result<(), String> {
    let pages = render_report_pages(result)?;
    let mut jpeg_pages = Vec::new();
    for page in pages {
        let mut bytes = Vec::new();
        JpegEncoder::new_with_quality(&mut bytes, 88)
            .encode_image(&DynamicImage::ImageRgb8(page))
            .map_err(|e| e.to_string())?;
        jpeg_pages.push(bytes);
    }

    let mut next_id = 3usize;
    let mut page_ids = Vec::new();
    let mut objects: Vec<(usize, Vec<u8>)> = Vec::new();
    for (index, jpeg) in jpeg_pages.iter().enumerate() {
        let image_id = next_id;
        let content_id = next_id + 1;
        let page_id = next_id + 2;
        next_id += 3;
        page_ids.push(page_id);

        let image_obj = format!(
            "<< /Type /XObject /Subtype /Image /Width 1240 /Height 1754 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            jpeg.len()
        );
        let mut image_bytes = image_obj.into_bytes();
        image_bytes.extend_from_slice(jpeg);
        image_bytes.extend_from_slice(b"\nendstream");
        objects.push((image_id, image_bytes));

        let content = format!("q\n595 0 0 842 0 0 cm\n/Im{} Do\nQ\n", index + 1);
        let content_obj = format!(
            "<< /Length {} >>\nstream\n{}endstream",
            content.len(),
            content
        );
        objects.push((content_id, content_obj.into_bytes()));

        let page_obj = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /XObject << /Im{} {} 0 R >> >> /Contents {} 0 R >>",
            index + 1,
            image_id,
            content_id
        );
        objects.push((page_id, page_obj.into_bytes()));
    }

    let kids = page_ids
        .iter()
        .map(|id| format!("{} 0 R", id))
        .collect::<Vec<_>>()
        .join(" ");
    let catalog = b"<< /Type /Catalog /Pages 2 0 R >>".to_vec();
    let pages_obj = format!(
        "<< /Type /Pages /Kids [{}] /Count {} >>",
        kids,
        page_ids.len()
    )
    .into_bytes();

    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    let mut offsets = vec![0u64; next_id];
    append_pdf_object(&mut pdf, &mut offsets, 1, &catalog);
    append_pdf_object(&mut pdf, &mut offsets, 2, &pages_obj);
    for (id, body) in objects {
        append_pdf_object(&mut pdf, &mut offsets, id, &body);
    }
    let xref_offset = pdf.len();
    write!(&mut pdf, "xref\n0 {}\n0000000000 65535 f \n", next_id).map_err(|e| e.to_string())?;
    for offset in offsets.iter().take(next_id).skip(1) {
        write!(&mut pdf, "{:010} 00000 n \n", offset).map_err(|e| e.to_string())?;
    }
    write!(
        &mut pdf,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        next_id, xref_offset
    )
    .map_err(|e| e.to_string())?;

    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(report_path, pdf).map_err(|e| e.to_string())
}

fn append_pdf_object(pdf: &mut Vec<u8>, offsets: &mut [u64], id: usize, body: &[u8]) {
    offsets[id] = pdf.len() as u64;
    pdf.extend_from_slice(format!("{} 0 obj\n", id).as_bytes());
    pdf.extend_from_slice(body);
    pdf.extend_from_slice(b"\nendobj\n");
}

fn render_report_pages(result: &SmartCullingTaskResult) -> Result<Vec<RgbImage>, String> {
    let font = load_report_font()?;
    let mut pages = vec![new_report_page()];
    let mut page_index = 0usize;
    let mut y = 70i32;

    draw_report_line(
        &mut pages,
        &mut page_index,
        &mut y,
        &font,
        40.0,
        "智能选图报告",
    );
    draw_report_line(
        &mut pages,
        &mut page_index,
        &mut y,
        &font,
        25.0,
        &format!("任务 ID: {}", result.task_id),
    );
    draw_report_line(
        &mut pages,
        &mut page_index,
        &mut y,
        &font,
        25.0,
        &format!("标注时间: {}", result.created_at),
    );
    draw_report_line(
        &mut pages,
        &mut page_index,
        &mut y,
        &font,
        25.0,
        &format!("任务状态: {}", status_label(&result.status)),
    );
    if result.preview_only {
        draw_report_line(
            &mut pages,
            &mut page_index,
            &mut y,
            &font,
            25.0,
            "预览结果，未写入照片",
        );
    }
    if let Some(error) = &result.error {
        draw_report_line(
            &mut pages,
            &mut page_index,
            &mut y,
            &font,
            25.0,
            &format!("失败原因: {}", error),
        );
    }
    draw_report_line(
        &mut pages,
        &mut page_index,
        &mut y,
        &font,
        25.0,
        &format!(
            "分析 {} 张，跳过 {} 张，精选 {} 张，待确认 {} 张，淘汰建议 {} 张，失败 {} 张",
            result.summary.analyzed,
            result.summary.skipped,
            result.summary.selected,
            result.summary.review,
            result.summary.reject_suggestion,
            result.summary.failed
        ),
    );
    y += 20;

    for item in &result.items {
        let line = format!(
            "{} | {}星 | {} | {} | {}",
            item.file_name,
            item.rating,
            status_label(&item.status),
            item.color_label.as_deref().unwrap_or("保留颜色"),
            item.reason_text
                .as_str()
                .if_empty(item.skip_reason.as_deref().unwrap_or("无原因"))
        );
        for wrapped in wrap_report_text(&line, 46) {
            draw_report_line(&mut pages, &mut page_index, &mut y, &font, 23.0, &wrapped);
        }
        y += 12;
    }

    Ok(pages.into_iter().map(rgba_to_rgb).collect())
}

trait EmptyStringFallback<'a> {
    fn if_empty(&'a self, fallback: &'a str) -> &'a str;
}

impl<'a> EmptyStringFallback<'a> for str {
    fn if_empty(&'a self, fallback: &'a str) -> &'a str {
        if self.is_empty() { fallback } else { self }
    }
}

fn new_report_page() -> RgbaImage {
    RgbaImage::from_pixel(1240, 1754, Rgba([255, 255, 255, 255]))
}

fn draw_report_line(
    pages: &mut Vec<RgbaImage>,
    page_index: &mut usize,
    y: &mut i32,
    font: &FontArc,
    size: f32,
    text: &str,
) {
    if *y > 1660 {
        pages.push(new_report_page());
        *page_index += 1;
        *y = 70;
    }
    draw_text_mut(
        &mut pages[*page_index],
        Rgba([28, 31, 36, 255]),
        70,
        *y,
        PxScale::from(size),
        font,
        text,
    );
    *y += (size * 1.45) as i32;
}

fn wrap_report_text(text: &str, max_units: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut units = 0usize;
    for ch in text.chars() {
        let width = if ch.is_ascii() { 1 } else { 2 };
        if units + width > max_units && !current.is_empty() {
            lines.push(current);
            current = String::new();
            units = 0;
        }
        current.push(ch);
        units += width;
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn rgba_to_rgb(image: RgbaImage) -> RgbImage {
    let mut rgb = RgbImage::new(image.width(), image.height());
    for (x, y, pixel) in image.enumerate_pixels() {
        rgb.put_pixel(x, y, Rgb([pixel[0], pixel[1], pixel[2]]));
    }
    rgb
}

fn load_report_font() -> Result<FontArc, String> {
    let candidates = [
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "/System/Library/Fonts/Supplemental/NISC18030.ttf",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ];
    for path in candidates {
        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        if let Ok(font) = FontArc::try_from_vec(bytes) {
            return Ok(font);
        }
    }
    Err("未找到可用于生成中文 PDF 的系统字体。".to_string())
}

fn status_label(status: &str) -> &'static str {
    match status {
        "selected" => "精选",
        "review" => "待确认",
        "reject_suggestion" => "淘汰建议",
        "skipped" => "跳过",
        "failed" => "失败",
        "applied" => "已应用",
        "revoked" => "已撤销",
        _ => "未知",
    }
}

fn emit_progress(app_handle: &AppHandle, task_id: &str, current: usize, total: usize, stage: &str) {
    let _ = app_handle.emit(
        "smart-culling:progress",
        SmartCullingProgress {
            task_id: task_id.to_string(),
            current,
            total,
            stage: stage.to_string(),
        },
    );
}
