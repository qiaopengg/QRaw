import type { LifecycleScreen, SmartCullingSnapshot, SmartCullingState } from './types';

const ANALYSIS_STATES = new Set<SmartCullingState>(['indexing', 'rendering', 'analyzing', 'organizing', 'cancelling']);

export function initialScreenForSnapshot(snapshot: SmartCullingSnapshot): LifecycleScreen {
  if (snapshot.state === 'unsupported') return 'unsupported';
  if (ANALYSIS_STATES.has(snapshot.state)) return 'analysis';
  if (snapshot.state === 'readyForReview') return 'review';
  if (snapshot.state === 'completed') return 'write';
  return 'setup';
}

export function screenForTaskTransition(
  currentScreen: LifecycleScreen,
  snapshot: SmartCullingSnapshot,
): LifecycleScreen {
  if (snapshot.state === 'unsupported') return 'unsupported';
  if (snapshot.state === 'completed') return 'write';

  if (ANALYSIS_STATES.has(snapshot.state)) {
    return 'analysis';
  }

  if (snapshot.state === 'readyForReview') return 'review';

  if (snapshot.state === 'idle') return 'setup';

  // Configuring snapshots are also used as command responses for key-person
  // detection. They update task data without taking navigation away from the
  // page the user is operating.
  return currentScreen;
}
