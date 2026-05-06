pub mod focus_areas;

#[tauri::command]
pub fn get_focus_regions(
    params: focus_areas::GetFocusRegionsParams,
) -> Result<Vec<focus_areas::FocusRegion>, String> {
    focus_areas::get_focus_regions(params)
}
