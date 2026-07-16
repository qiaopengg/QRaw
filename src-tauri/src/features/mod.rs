pub mod focus_areas;
pub mod smart_culling;

#[tauri::command]
pub fn get_focus_regions(
    params: focus_areas::GetFocusRegionsParams,
) -> Result<Vec<focus_areas::FocusRegion>, String> {
    focus_areas::get_focus_regions(params)
}

#[tauri::command]
pub async fn smart_culling_command(
    request: smart_culling::SmartCullingRequest,
    app_handle: tauri::AppHandle,
) -> Result<smart_culling::SmartCullingSnapshot, smart_culling::SmartCullingCommandError> {
    smart_culling::smart_culling_command(request, app_handle).await
}
