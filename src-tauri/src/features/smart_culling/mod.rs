mod analysis;
mod api;
mod coordinator;
mod coordinator_session;
mod coordinator_support;
pub(crate) mod domain;
mod expression;
mod face_geometry;
mod face_identity;
mod face_models;
#[cfg(any(test, all(debug_assertions, target_os = "macos")))]
mod face_motion_poc;
mod grouping;
pub(crate) mod infrastructure;
mod key_person_policy;
mod key_person_scoring;
mod mode_policy;
mod mode_scoring;
mod models;
mod preflight;
mod quality_evidence;
mod review_policy;
mod runner;
mod scoring;
mod task_recovery;
mod types;

use tauri::AppHandle;

pub use api::{SmartCullingCommandError, SmartCullingRequest, SmartCullingSnapshot};

pub async fn smart_culling_command(
    request: SmartCullingRequest,
    app_handle: AppHandle,
) -> Result<SmartCullingSnapshot, SmartCullingCommandError> {
    let failure_code = format!("{}_failed", request.action_name());
    tauri::async_runtime::spawn_blocking(move || coordinator::handle(request, app_handle))
        .await
        .map_err(|error| {
            SmartCullingCommandError::new(
                "gateway_failed",
                format!("Smart-culling gateway task failed: {error}"),
            )
        })?
        .map_err(|detail| SmartCullingCommandError::new(failure_code, detail))
}
