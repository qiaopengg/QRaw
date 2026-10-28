import { create } from 'zustand';
import { invokeSmartCulling, normalizeSmartCullingError } from './api';
import type {
  KeyPersonSelection,
  LifecycleScreen,
  SmartCullingCommandError,
  SmartCullingRequest,
  SmartCullingSnapshot,
} from './types';
import type { SmartCullingMode } from './constants';
import { ANALYSIS_STATES, initialScreenForSnapshot, screenForTaskTransition } from './navigation';

interface SmartCullingState {
  snapshot: SmartCullingSnapshot | null;
  screen: LifecycleScreen;
  mode: SmartCullingMode;
  keyPeople: KeyPersonSelection[];
  focusedResultId: string | null;
  confirmOpen: boolean;
  abandonOpen: boolean;
  cancelOpen: boolean;
  manualSyncPending: boolean;
  busy: boolean;
  error: SmartCullingCommandError | null;
  setState: (update: Partial<SmartCullingState>) => void;
}

export const useSmartCullingStore = create<SmartCullingState>((set) => ({
  snapshot: null,
  screen: 'setup',
  mode: 'auto',
  keyPeople: [],
  focusedResultId: null,
  confirmOpen: false,
  abandonOpen: false,
  cancelOpen: false,
  manualSyncPending: false,
  busy: false,
  error: null,
  setState: (update) => set(update),
}));

export function applySmartCullingEvent(snapshot: SmartCullingSnapshot) {
  const current = useSmartCullingStore.getState();
  if (isOlderSnapshot(snapshot, current.snapshot)) return;
  const clearPeople = ['readyForReview', 'completed', 'unsupported', 'failed'].includes(snapshot.state);
  current.setState({
    snapshot,
    screen: screenForTaskTransition(current.screen, snapshot),
    ...(clearPeople ? { keyPeople: [] } : {}),
  });
}

function isOlderSnapshot(incoming: SmartCullingSnapshot, current: SmartCullingSnapshot | null) {
  return current !== null && incoming.revision < current.revision;
}

// Backend commands can mutate the same task. Preserve the user's order rather
// than merely hiding stale responses after the older mutation already landed.
let commandQueue: Promise<void> = Promise.resolve();
let pendingCommands = 0;

export function runSmartCullingCommand(request: SmartCullingRequest, preserveScreen = false) {
  pendingCommands += 1;
  useSmartCullingStore.getState().setState({ busy: true });
  // A running task must remain cancellable while a library write is pending.
  // Before a running state is observed, cancellation waits for start.
  const state = useSmartCullingStore.getState().snapshot?.state;
  const cancelRunning = request.action === 'cancel' && state !== undefined && ANALYSIS_STATES.has(state);
  const result = cancelRunning
    ? executeCommand(request, preserveScreen)
    : commandQueue.then(() => executeCommand(request, preserveScreen));
  const settled = result.then(
    () => undefined,
    () => undefined,
  );
  commandQueue = cancelRunning ? Promise.all([commandQueue, settled]).then(() => undefined) : settled;
  return result.finally(() => {
    pendingCommands -= 1;
    useSmartCullingStore.getState().setState({ busy: pendingCommands > 0 });
  });
}

async function executeCommand(request: SmartCullingRequest, preserveScreen: boolean) {
  useSmartCullingStore.getState().setState({ error: null });
  try {
    const snapshot = await invokeSmartCulling(request);
    const clearPeople = request.action === 'start' || request.action === 'abandon';
    const current = useSmartCullingStore.getState();
    if (isOlderSnapshot(snapshot, current.snapshot)) return snapshot;
    const focusedResultId = snapshot.results.some((result) => result.resultId === current.focusedResultId)
      ? current.focusedResultId
      : (snapshot.results[0]?.resultId ?? null);
    current.setState({
      snapshot,
      screen: preserveScreen ? current.screen : screenForTaskTransition(current.screen, snapshot),
      // Folder preflight returns its default mode after an asynchronous model
      // check. Keep a shooting mode the user selected while that check ran.
      mode:
        request.action === 'detectPeople' || request.action === 'inspect'
          ? current.mode
          : (snapshot.mode ?? current.mode),
      keyPeople: clearPeople ? [] : current.keyPeople,
      focusedResultId,
    });
    return snapshot;
  } catch (error) {
    useSmartCullingStore.getState().setState({ error: normalizeSmartCullingError(error) });
    throw error;
  }
}

export { initialScreenForSnapshot };
