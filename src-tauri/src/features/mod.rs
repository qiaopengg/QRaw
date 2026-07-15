pub mod focus_areas;
pub mod smart_culling;

#[tauri::command]
pub fn get_focus_regions(
    params: focus_areas::GetFocusRegionsParams,
) -> Result<Vec<focus_areas::FocusRegion>, String> {
    focus_areas::get_focus_regions(params)
}

#[tauri::command]
pub async fn smart_culling_analyze(
    paths: Vec<String>,
    settings: smart_culling::SmartCullingSettings,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<smart_culling::SmartCullingSuggestions, String> {
    smart_culling::smart_culling_analyze(paths, settings, app_handle, state).await
}

#[tauri::command]
pub fn smart_culling_write_metadata(
    items: Vec<smart_culling::SmartCullingApplyItem>,
) -> Result<(), String> {
    smart_culling::smart_culling_write_metadata(items)
}
