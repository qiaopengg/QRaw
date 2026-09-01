use std::sync::Arc;

use tauri::{AppHandle, Manager};

use super::api::{CapabilityLevel, DevicePreflight, SmartCullingCapabilities};
use super::models::{SmartCullingFaceModels, load_face_models};
use super::scoring::{MODEL_VERSION, POLICY_VERSION};
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
        policy_version: POLICY_VERSION.to_string(),
        capabilities: SmartCullingCapabilities::default(),
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

    #[cfg(all(debug_assertions, target_os = "macos"))]
    let calibration_model_dir = match super::calibration_model_assets::resolve(app_handle) {
        Ok(path) => path,
        Err(error) => {
            log::warn!("Smart-culling calibration model resolution failed: {error}");
            return Err(unsupported("calibration_models_missing"));
        }
    };

    #[cfg(all(debug_assertions, target_os = "macos"))]
    if let Err(error) =
        super::face_motion_poc::preflight_calibration_models_from(&calibration_model_dir)
    {
        log::warn!("Smart-culling eye/expression calibration preflight failed: {error}");
        return Err(unsupported("gpu_inference_unavailable"));
    }

    #[cfg(all(debug_assertions, target_os = "macos"))]
    if let Err(error) =
        super::expression_quality_poc::preflight_calibration_models_from(&calibration_model_dir)
    {
        log::warn!("Smart-culling expression-quality calibration preflight failed: {error}");
        return Err(unsupported("gpu_inference_unavailable"));
    }

    let vision_observation_available = vision_observation_available();

    Ok((
        DevicePreflight {
            checked: true,
            supported: true,
            platform,
            provider,
            model_version: MODEL_VERSION.to_string(),
            policy_version: POLICY_VERSION.to_string(),
            capabilities: current_capabilities(vision_observation_available),
            reason: None,
        },
        models,
    ))
}

fn vision_observation_available() -> bool {
    #[cfg(all(debug_assertions, target_os = "macos"))]
    {
        return match super::vision_quality_poc::preflight_calibration_models() {
            Ok(()) => true,
            Err(error) => {
                // Observation-only evidence must not disable the independently
                // calibrated eye and expression path on macOS 13/14.
                log::warn!("Smart-culling Apple Vision quality observation unavailable: {error}");
                false
            }
        };
    }
    #[allow(unreachable_code)]
    false
}

fn current_capabilities(vision_observation_available: bool) -> SmartCullingCapabilities {
    #[cfg(all(debug_assertions, target_os = "macos"))]
    let (eye_state, expression) = (CapabilityLevel::Calibration, CapabilityLevel::Calibration);
    #[cfg(not(all(debug_assertions, target_os = "macos")))]
    let (eye_state, expression) = (CapabilityLevel::Unavailable, CapabilityLevel::Unavailable);

    SmartCullingCapabilities {
        eye_state,
        expression,
        person_clarity: if vision_observation_available {
            CapabilityLevel::Calibration
        } else {
            CapabilityLevel::Conservative
        },
        optical_quality: CapabilityLevel::Conservative,
        composition: if vision_observation_available {
            CapabilityLevel::ObservationOnly
        } else {
            CapabilityLevel::Unavailable
        },
        key_person_identity: CapabilityLevel::ManualOnly,
        release_ready: false,
    }
}

fn provider_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        return "Core ML MLProgram (no ORT CPU EP fallback)";
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

#[cfg(test)]
mod tests {
    use super::current_capabilities;
    use crate::features::smart_culling::api::CapabilityLevel;

    #[test]
    fn incomplete_evidence_never_claims_release_readiness() {
        let without_vision = current_capabilities(false);
        assert!(!without_vision.release_ready);
        assert_eq!(without_vision.composition, CapabilityLevel::Unavailable);
        assert_eq!(
            without_vision.key_person_identity,
            CapabilityLevel::ManualOnly
        );

        let with_vision = current_capabilities(true);
        assert!(!with_vision.release_ready);
        assert_eq!(with_vision.person_clarity, CapabilityLevel::Calibration);
        assert_eq!(with_vision.composition, CapabilityLevel::ObservationOnly);
    }
}
