#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TaskState {
    Idle,
    Preflighting,
    Configuring,
    Indexing,
    Rendering,
    Analyzing,
    Organizing,
    ReadyForReview,
    Confirming,
    Completed,
    Cancelling,
    Abandoning,
    Failed,
    Unsupported,
}

impl TaskState {
    pub(crate) fn can_transition_to(self, next: Self) -> bool {
        use TaskState::*;

        matches!(
            (self, next),
            (Idle, Preflighting)
                | (Preflighting, Configuring | Unsupported | Failed)
                | (Configuring, Indexing | Abandoning)
                | (Indexing, Rendering | Cancelling | Failed)
                | (Rendering, Analyzing | Cancelling | Failed)
                | (Analyzing, Organizing | Cancelling | Failed)
                | (Organizing, ReadyForReview | Cancelling | Failed)
                | (Cancelling, ReadyForReview | Failed)
                | (ReadyForReview, Confirming | Abandoning)
                | (Confirming, Completed | ReadyForReview | Failed)
                | (Completed | Abandoning | Failed | Unsupported, Idle)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::TaskState;

    #[test]
    fn follows_the_review_before_confirming_path() {
        assert!(TaskState::Organizing.can_transition_to(TaskState::ReadyForReview));
        assert!(TaskState::ReadyForReview.can_transition_to(TaskState::Confirming));
        assert!(!TaskState::Analyzing.can_transition_to(TaskState::Confirming));
    }

    #[test]
    fn cancellation_keeps_completed_work_for_review() {
        assert!(TaskState::Analyzing.can_transition_to(TaskState::Cancelling));
        assert!(TaskState::Cancelling.can_transition_to(TaskState::ReadyForReview));
        assert!(!TaskState::Cancelling.can_transition_to(TaskState::Completed));
    }

    #[test]
    fn abandoning_never_transitions_to_confirming() {
        assert!(TaskState::ReadyForReview.can_transition_to(TaskState::Abandoning));
        assert!(TaskState::Abandoning.can_transition_to(TaskState::Idle));
        assert!(!TaskState::Abandoning.can_transition_to(TaskState::Confirming));
    }
}
