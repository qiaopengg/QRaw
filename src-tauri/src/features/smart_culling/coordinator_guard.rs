use std::path::Path;

use super::api::SmartCullingSnapshot;
use super::coordinator_session::TaskSession;
use super::domain::TaskState;

pub(super) fn existing_inspection_snapshot(
    session: Option<&TaskSession>,
    requested_root: &Path,
) -> Result<Option<SmartCullingSnapshot>, String> {
    let Some(session) = session else {
        return Ok(None);
    };
    Ok(
        inspection_must_reuse(session.state, &session.root_path, requested_root)?
            .then(|| session.snapshot()),
    )
}

fn inspection_must_reuse(
    state: TaskState,
    current_root: &Path,
    requested_root: &Path,
) -> Result<bool, String> {
    if !matches!(
        state,
        TaskState::Indexing
            | TaskState::Rendering
            | TaskState::Analyzing
            | TaskState::Organizing
            | TaskState::Cancelling
            | TaskState::ReadyForReview
            | TaskState::Confirming
    ) {
        return Ok(false);
    }
    if current_root != requested_root {
        return Err(format!(
            "A smart-culling task for {} is still active; review or abandon it before opening {}",
            current_root.display(),
            requested_root.display()
        ));
    }
    Ok(true)
}

pub(super) fn ensure_write_report_matches(
    current_task_id: &str,
    state: TaskState,
    writing_task_id: &str,
) -> Result<(), String> {
    if current_task_id != writing_task_id || state != TaskState::Confirming {
        return Err(
            "Smart-culling task changed while writing; the write report was not applied"
                .to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspection_rechecks_the_state_after_preflight_before_replacing_a_session() {
        let root = Path::new("/photos/current");
        assert!(!inspection_must_reuse(TaskState::Completed, root, root).unwrap());
        assert!(inspection_must_reuse(TaskState::Confirming, root, root).unwrap());
        assert!(
            inspection_must_reuse(TaskState::Confirming, root, Path::new("/photos/other")).is_err()
        );
    }

    #[test]
    fn inspection_keeps_review_and_running_tasks_but_allows_idle_configuration() {
        let root = Path::new("/photos/current");
        for state in [
            TaskState::ReadyForReview,
            TaskState::Analyzing,
            TaskState::Cancelling,
        ] {
            assert!(inspection_must_reuse(state, root, root).unwrap());
        }
        assert!(!inspection_must_reuse(TaskState::Configuring, root, root).unwrap());
    }

    #[test]
    fn write_report_only_updates_its_own_confirming_session() {
        assert!(ensure_write_report_matches("original", TaskState::Confirming, "original").is_ok());
        assert!(
            ensure_write_report_matches("replacement", TaskState::Confirming, "original").is_err()
        );
        assert!(ensure_write_report_matches("original", TaskState::Completed, "original").is_err());
        assert!(
            ensure_write_report_matches("original", TaskState::Configuring, "original").is_err()
        );
    }
}
