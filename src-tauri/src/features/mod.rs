pub mod focus_areas;
pub mod smart_culling;

#[tauri::command]
pub fn get_focus_regions(
    params: focus_areas::GetFocusRegionsParams,
) -> Result<Vec<focus_areas::FocusRegion>, String> {
    focus_areas::get_focus_regions(params)
}

#[tauri::command]
pub fn smart_culling_check_models(
    app_handle: tauri::AppHandle,
) -> Result<smart_culling::SmartCullingModelsStatus, String> {
    smart_culling::smart_culling_check_models(app_handle)
}

#[tauri::command]
pub fn smart_culling_open_models_dir(app_handle: tauri::AppHandle) -> Result<String, String> {
    smart_culling::smart_culling_open_models_dir(app_handle)
}

#[tauri::command]
pub async fn smart_culling_download_models(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<smart_culling::SmartCullingModelsStatus, String> {
    smart_culling::smart_culling_download_models(app_handle, state).await
}

#[tauri::command]
pub async fn smart_culling_start_task(
    params: smart_culling::SmartCullingStartParams,
    app_handle: tauri::AppHandle,
) -> Result<smart_culling::SmartCullingStartResponse, String> {
    smart_culling::smart_culling_start_task(params, app_handle).await
}

#[tauri::command]
pub fn smart_culling_cancel_task(task_id: String) -> Result<(), String> {
    smart_culling::smart_culling_cancel_task(task_id)
}

#[tauri::command]
pub fn smart_culling_get_task_result(
    task_id: String,
    app_handle: tauri::AppHandle,
) -> Result<smart_culling::SmartCullingTaskResult, String> {
    smart_culling::smart_culling_get_task_result(task_id, app_handle)
}

#[tauri::command]
pub fn smart_culling_discard_task_result(
    task_id: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    smart_culling::smart_culling_discard_task_result(task_id, app_handle)
}

#[tauri::command]
pub fn smart_culling_apply_task_result(
    task_id: String,
    items: Vec<smart_culling::SmartCullingReviewItem>,
    app_handle: tauri::AppHandle,
) -> Result<smart_culling::SmartCullingApplyResult, String> {
    smart_culling::smart_culling_apply_task_result(task_id, items, app_handle)
}

#[tauri::command]
pub fn smart_culling_list_recent_tasks(
    app_handle: tauri::AppHandle,
) -> Result<Vec<smart_culling::SmartCullingHistoryItem>, String> {
    smart_culling::smart_culling_list_recent_tasks(app_handle)
}

#[tauri::command]
pub fn smart_culling_list_presets(
    app_handle: tauri::AppHandle,
) -> Result<Vec<smart_culling::SmartCullingUserPreset>, String> {
    smart_culling::smart_culling_list_presets(app_handle)
}

#[tauri::command]
pub fn smart_culling_save_preset(
    params: smart_culling::SmartCullingSavePresetParams,
    app_handle: tauri::AppHandle,
) -> Result<smart_culling::SmartCullingUserPreset, String> {
    smart_culling::smart_culling_save_preset(params, app_handle)
}

#[tauri::command]
pub fn smart_culling_delete_preset(id: String, app_handle: tauri::AppHandle) -> Result<(), String> {
    smart_culling::smart_culling_delete_preset(id, app_handle)
}

#[tauri::command]
pub fn smart_culling_export_report_pdf(
    params: smart_culling::SmartCullingExportReportParams,
    app_handle: tauri::AppHandle,
) -> Result<smart_culling::SmartCullingReportResult, String> {
    smart_culling::smart_culling_export_report_pdf(params, app_handle)
}

#[tauri::command]
pub fn smart_culling_undo_task(
    task_id: String,
    app_handle: tauri::AppHandle,
) -> Result<smart_culling::SmartCullingUndoResult, String> {
    smart_culling::smart_culling_undo_task(task_id, app_handle)
}
