use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::api::SmartCullingSnapshot;

const MARKER_FILE: &str = "smart-culling/interrupted-task.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct InterruptedTask {
    pub task_id: String,
    pub root_path: String,
    pub state: String,
}

pub(crate) fn remember(snapshot: &SmartCullingSnapshot, app_handle: &AppHandle) {
    let (Some(task_id), Some(root_path)) = (&snapshot.task_id, &snapshot.root_path) else {
        return;
    };
    let marker = InterruptedTask {
        task_id: task_id.clone(),
        root_path: root_path.clone(),
        state: snapshot.state.clone(),
    };
    if let Err(error) = write_marker(app_handle, &marker) {
        log::warn!("Could not persist the smart-culling interruption marker: {error}");
    }
}

pub(crate) fn forget(app_handle: &AppHandle) {
    let Ok(path) = marker_path(app_handle) else {
        return;
    };
    if let Err(error) = fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        log::warn!("Could not clear the smart-culling interruption marker: {error}");
    }
}

pub(crate) fn take(app_handle: &AppHandle) -> Option<InterruptedTask> {
    let path = marker_path(app_handle).ok()?;
    let marker = fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<InterruptedTask>(&bytes).ok());
    if let Err(error) = fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        log::warn!("Could not consume the smart-culling interruption marker: {error}");
    }
    marker
}

fn write_marker(app_handle: &AppHandle, marker: &InterruptedTask) -> Result<(), String> {
    let path = marker_path(app_handle)?;
    let parent = path
        .parent()
        .ok_or_else(|| "Smart-culling marker path has no parent".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec(marker).map_err(|error| error.to_string())?;
    fs::write(path, bytes).map_err(|error| error.to_string())
}

fn marker_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    app_handle
        .path()
        .app_data_dir()
        .map(|path| path.join(MARKER_FILE))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::InterruptedTask;

    #[test]
    fn interruption_marker_round_trips_without_task_results() {
        let marker = InterruptedTask {
            task_id: "task".to_string(),
            root_path: "/photos".to_string(),
            state: "readyForReview".to_string(),
        };

        let encoded = serde_json::to_vec(&marker).unwrap();
        let decoded = serde_json::from_slice::<InterruptedTask>(&encoded).unwrap();

        assert_eq!(decoded.task_id, marker.task_id);
        assert_eq!(decoded.root_path, marker.root_path);
        assert_eq!(decoded.state, marker.state);
    }
}
