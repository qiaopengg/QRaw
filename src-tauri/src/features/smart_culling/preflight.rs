use std::sync::Arc;

use tauri::{AppHandle, Manager};

use super::api::DevicePreflight;
use super::models::{SmartCullingFaceModels, load_face_models};
use super::scoring::MODEL_VERSION;
use crate::AppState;
use crate::gpu_processing::get_or_init_gpu_context;

pub(crate) fn run_preflight(
    app_handle: &AppHandle,
) -> Result<(DevicePreflight, Arc<SmartCullingFaceModels>), DevicePreflight> {
    let platform = std::env::consts::OS.to_string();
    let provider = provider_name().to_string();
    let unsupported = |reason: &str| DevicePreflight {
        checked: true,
        supported: false,
        platform: platform.clone(),
        provider: provider.clone(),
        model_version: MODEL_VERSION.to_string(),
        reason: Some(reason.to_string()),
    };

    if !platform_candidate_is_supported() {
        return Err(unsupported("unsupported_platform"));
    }

    let state = app_handle.state::<AppState>();
    if let Err(error) = get_or_init_gpu_context(&state, app_handle) {
        log::warn!("Smart-culling GPU render preflight failed: {error}");
        return Err(unsupported("gpu_rendering_unavailable"));
    }

    let models = match load_face_models(app_handle) {
        Ok(models) => models,
        Err(error) => {
            log::warn!("Smart-culling model preflight failed: {error}");
            let reason = if error.to_string().contains("integrity check failed") {
                "bundled_models_invalid"
            } else if error.to_string().contains("cannot be read") {
                "bundled_models_missing"
            } else {
                "gpu_inference_unavailable"
            };
            return Err(unsupported(reason));
        }
    };

    Ok((
        DevicePreflight {
            checked: true,
            supported: true,
            platform,
            provider,
            model_version: MODEL_VERSION.to_string(),
            reason: None,
        },
        models,
    ))
}

fn provider_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        return "Core ML GPU accelerated";
    }
    #[cfg(target_os = "windows")]
    {
        return "DirectML";
    }
    #[allow(unreachable_code)]
    "Unsupported"
}

fn platform_candidate_is_supported() -> bool {
    cfg!(all(target_os = "macos", target_arch = "aarch64"))
        || cfg!(all(target_os = "windows", target_arch = "x86_64"))
}
