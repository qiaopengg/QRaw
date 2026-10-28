//! Order snapshots at capture time, before asynchronous commands/events can overtake them.

use std::sync::atomic::{AtomicU64, Ordering};

use super::api::SmartCullingSnapshot;

static NEXT_REVISION: AtomicU64 = AtomicU64::new(1);

// Session snapshots must be captured while holding the coordinator mutex.
// Revisions span task replacement and abandonment, not only one task's progress.
pub(super) fn next_revision() -> u64 {
    NEXT_REVISION.fetch_add(1, Ordering::Relaxed)
}

pub(super) fn store_if_current(
    current: &mut SmartCullingSnapshot,
    incoming: &SmartCullingSnapshot,
) -> bool {
    if incoming.revision < current.revision {
        return false;
    }
    *current = incoming.clone();
    true
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, mpsc};

    use super::*;

    fn snapshot(state: &str, task_id: Option<&str>) -> SmartCullingSnapshot {
        SmartCullingSnapshot {
            state: state.to_string(),
            task_id: task_id.map(str::to_string),
            ..SmartCullingSnapshot::default()
        }
    }

    #[test]
    fn delayed_progress_cannot_replace_a_published_cancellation() {
        let current = Arc::new(Mutex::new(snapshot("indexing", Some("task"))));
        let writer_current = Arc::clone(&current);
        let (captured_tx, captured_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let progress = {
                let _coordinator = writer_current.lock().unwrap();
                snapshot("analyzing", Some("task"))
            };
            captured_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            store_if_current(&mut writer_current.lock().unwrap(), &progress)
        });
        captured_rx.recv().unwrap();
        {
            let mut current = current.lock().unwrap();
            let cancellation = snapshot("cancelling", Some("task"));
            assert!(store_if_current(&mut current, &cancellation));
        }
        release_tx.send(()).unwrap();
        let accepted = worker.join().unwrap();

        assert_eq!(current.lock().unwrap().state, "cancelling");
        assert!(!accepted, "a stale progress snapshot must not be published");
    }

    #[test]
    fn delayed_previous_task_cannot_replace_abandonment_or_a_new_task() {
        let old = snapshot("readyForReview", Some("old"));
        let mut current = SmartCullingSnapshot::default();
        assert!(!store_if_current(&mut current, &old));
        assert_eq!(current.state, "idle");
        assert!(current.task_id.is_none());

        let new = snapshot("configuring", Some("new"));
        assert!(store_if_current(&mut current, &new));
        assert!(!store_if_current(&mut current, &old));
        assert_eq!(current.task_id.as_deref(), Some("new"));
    }

    #[test]
    fn snapshot_revisions_are_serialized_and_clones_keep_the_capture_order() {
        let earlier = SmartCullingSnapshot::default();
        let later = SmartCullingSnapshot::default();
        assert!(later.revision > earlier.revision);
        assert_eq!(earlier.clone().revision, earlier.revision);
        assert_eq!(
            serde_json::to_value(&later).unwrap()["revision"],
            later.revision
        );
        let mut current = later.clone();
        assert!(store_if_current(&mut current, &later));
    }
}
