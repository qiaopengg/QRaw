use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::api::SmartCullingSnapshot;

const MARKER_FILE: &str = "smart-culling/interrupted-task.json";
static MARKERS: MarkerStore = MarkerStore {
    latest_revision: Mutex::new(0),
};

#[derive(Default)]
struct MarkerStore {
    latest_revision: Mutex<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct InterruptedTask {
    pub task_id: String,
    pub root_path: String,
    pub state: String,
}

pub(crate) fn remember(snapshot: &SmartCullingSnapshot, app_handle: &AppHandle) {
    if let Err(error) = marker_path(app_handle).and_then(|path| MARKERS.remember(&path, snapshot)) {
        log::warn!("Could not persist the smart-culling interruption marker: {error}");
    }
}

pub(crate) fn forget(snapshot: &SmartCullingSnapshot, app_handle: &AppHandle) {
    if let Err(error) = marker_path(app_handle).and_then(|path| MARKERS.forget(&path, snapshot)) {
        log::warn!("Could not clear the smart-culling interruption marker: {error}");
    }
}

pub(crate) fn take(
    snapshot: &SmartCullingSnapshot,
    app_handle: &AppHandle,
) -> Option<InterruptedTask> {
    match marker_path(app_handle).and_then(|path| MARKERS.take(&path, snapshot)) {
        Ok(marker) => marker,
        Err(error) => {
            log::warn!("Could not consume the smart-culling interruption marker: {error}");
            None
        }
    }
}

impl MarkerStore {
    fn remember(&self, path: &Path, snapshot: &SmartCullingSnapshot) -> Result<(), String> {
        let (Some(task_id), Some(root_path)) = (&snapshot.task_id, &snapshot.root_path) else {
            return Ok(());
        };
        let marker = InterruptedTask {
            task_id: task_id.clone(),
            root_path: root_path.clone(),
            state: snapshot.state.clone(),
        };
        self.apply(snapshot.revision, || write_marker(path, &marker))?;
        Ok(())
    }

    fn forget(&self, path: &Path, snapshot: &SmartCullingSnapshot) -> Result<(), String> {
        self.apply(snapshot.revision, || remove_marker(path))?;
        Ok(())
    }

    fn take(
        &self,
        path: &Path,
        snapshot: &SmartCullingSnapshot,
    ) -> Result<Option<InterruptedTask>, String> {
        self.apply(snapshot.revision, || {
            let bytes = match fs::read(path) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error.to_string()),
            };
            let marker = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            remove_marker(path)?;
            Ok(Some(marker))
        })
        .map(Option::flatten)
    }

    fn apply<T>(
        &self,
        revision: u64,
        action: impl FnOnce() -> Result<T, String>,
    ) -> Result<Option<T>, String> {
        // Never acquire the coordinator from here: callers may already hold it.
        let mut latest = self
            .latest_revision
            .lock()
            .map_err(|_| "Interruption marker state is poisoned; operation stopped".to_string())?;
        if revision <= *latest {
            return Ok(None);
        }
        // A cleared or failed newer operation is still a barrier: delayed
        // writes must not revive an obsolete task, including an exact replay.
        *latest = revision;
        action().map(Some)
    }
}

fn write_marker(path: &Path, marker: &InterruptedTask) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Smart-culling marker path has no parent".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec(marker).map_err(|error| error.to_string())?;
    fs::write(path, bytes).map_err(|error| error.to_string())
}

fn remove_marker(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn marker_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    app_handle
        .path()
        .app_data_dir()
        .map(|path| path.join(MARKER_FILE))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "task_recovery_tests.rs"]
mod tests;
