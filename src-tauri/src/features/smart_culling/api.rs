use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "action"
)]
pub enum SmartCullingRequest {
    Status,
    Inspect {
        root_path: String,
    },
    DetectPeople {
        path: String,
    },
    Start {
        root_path: String,
        mode: String,
        #[serde(default)]
        key_people: Vec<KeyPersonSelection>,
    },
    Cancel,
    UpdateReview {
        changes: Vec<ReviewChange>,
    },
    Confirm,
    RetryFailures,
    ReconcileManual {
        paths: Vec<String>,
    },
    Abandon,
}

impl SmartCullingRequest {
    pub(crate) fn action_name(&self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Inspect { .. } => "inspect",
            Self::DetectPeople { .. } => "detect_people",
            Self::Start { .. } => "start",
            Self::Cancel => "cancel",
            Self::UpdateReview { .. } => "update_review",
            Self::Confirm => "confirm",
            Self::RetryFailures => "retry_failures",
            Self::ReconcileManual { .. } => "reconcile_manual",
            Self::Abandon => "abandon",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingCommandError {
    pub code: String,
    pub detail: String,
}

impl SmartCullingCommandError {
    pub(crate) fn new(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            detail: detail.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyPersonSelection {
    pub sample_path: String,
    pub bbox: [f32; 4],
    pub priority: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewChange {
    pub result_id: String,
    pub adopted: bool,
    pub rating: u8,
    pub color_label: Option<String>,
    pub mode: String,
    pub edited: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartCullingSnapshot {
    pub task_id: Option<String>,
    pub state: String,
    pub root_path: Option<String>,
    pub mode: Option<String>,
    pub device: DevicePreflight,
    pub inventory: InventorySummary,
    pub progress: TaskProgress,
    pub results: Vec<ReviewResult>,
    pub failures: Vec<FailureItem>,
    pub detected_image_path: Option<String>,
    pub detected_faces: Vec<DetectedFaceDto>,
    pub write_summary: Option<WriteSummary>,
}

impl Default for SmartCullingSnapshot {
    fn default() -> Self {
        Self {
            task_id: None,
            state: "idle".to_string(),
            root_path: None,
            mode: None,
            device: DevicePreflight::default(),
            inventory: InventorySummary::default(),
            progress: TaskProgress::default(),
            results: Vec::new(),
            failures: Vec::new(),
            detected_image_path: None,
            detected_faces: Vec::new(),
            write_summary: None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicePreflight {
    pub checked: bool,
    pub supported: bool,
    pub platform: String,
    pub provider: String,
    pub model_version: String,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorySummary {
    pub total_assets: usize,
    pub eligible_assets: usize,
    pub protected_assets: usize,
    pub skipped_assets: usize,
    pub failed_assets: usize,
    pub folder_count: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub completed: usize,
    pub total: usize,
    pub percent: u8,
    pub stage: String,
    pub eta_seconds: Option<u64>,
    pub partial: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewResult {
    pub result_id: String,
    pub path: String,
    pub member_paths: Vec<String>,
    pub folder: String,
    pub story: String,
    pub group_id: String,
    pub group_size: usize,
    pub recommended_count: usize,
    pub rating: u8,
    pub color_label: Option<String>,
    pub source: String,
    pub mode: String,
    pub reason_codes: Vec<String>,
    pub confidence: f32,
    pub adopted: bool,
    pub protected: bool,
    pub width: u32,
    pub height: u32,
    pub faces: Vec<DetectedFaceDto>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedFaceDto {
    pub bbox: [f32; 4],
    pub score: f32,
    pub thumbnail_data_url: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailureItem {
    pub path: String,
    pub member_paths: Vec<String>,
    pub stage: String,
    pub code: String,
    pub detail: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteSummary {
    pub succeeded: usize,
    pub failed: usize,
    pub protected: usize,
    pub skipped: usize,
    pub succeeded_paths: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::{SmartCullingCommandError, SmartCullingRequest};

    #[test]
    fn gateway_accepts_frontend_camel_case_fields() {
        let request = serde_json::from_value::<SmartCullingRequest>(serde_json::json!({
            "action": "inspect",
            "rootPath": "/photos"
        }))
        .expect("frontend request should deserialize");

        assert!(
            matches!(request, SmartCullingRequest::Inspect { root_path } if root_path == "/photos")
        );
    }

    #[test]
    fn command_error_has_a_stable_frontend_envelope() {
        let value = serde_json::to_value(SmartCullingCommandError::new(
            "inspect_failed",
            "folder is unavailable",
        ))
        .unwrap();

        assert_eq!(value["code"], "inspect_failed");
        assert_eq!(value["detail"], "folder is unavailable");
    }
}
