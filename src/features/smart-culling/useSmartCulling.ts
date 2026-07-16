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

interface SmartCullingState {
  snapshot: SmartCullingSnapshot | null;
  screen: LifecycleScreen;
  mode: SmartCullingMode;
  keyPeople: KeyPersonSelection[];
  focusedResultId: string | null;
  confirmOpen: boolean;
  abandonOpen: boolean;
  cancelOpen: boolean;
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
  busy: false,
  error: null,
  setState: (update) => set(update),
}));

export function screenForSnapshot(snapshot: SmartCullingSnapshot): LifecycleScreen {
  if (snapshot.state === 'unsupported') return 'unsupported';
  if (snapshot.state === 'configuring') return 'setup';
  if (['indexing', 'rendering', 'analyzing', 'organizing', 'cancelling'].includes(snapshot.state)) return 'analysis';
  if (snapshot.state === 'readyForReview') return 'ready';
  if (snapshot.state === 'completed') return 'write';
  return 'setup';
}

export async function runSmartCullingCommand(request: SmartCullingRequest, preserveScreen = false) {
  const store = useSmartCullingStore.getState();
  store.setState({ busy: true, error: null });
  try {
    const snapshot = await invokeSmartCulling(request);
    const clearPeople = request.action === 'start' || request.action === 'abandon';
    const current = useSmartCullingStore.getState();
    const focusedResultId = snapshot.results.some((result) => result.resultId === current.focusedResultId)
      ? current.focusedResultId
      : (snapshot.results[0]?.resultId ?? null);
    current.setState({
      snapshot,
      screen: preserveScreen ? current.screen : screenForSnapshot(snapshot),
      mode: snapshot.mode ?? current.mode,
      keyPeople: clearPeople ? [] : current.keyPeople,
      focusedResultId,
      busy: false,
    });
    return snapshot;
  } catch (error) {
    useSmartCullingStore.getState().setState({ busy: false, error: normalizeSmartCullingError(error) });
    throw error;
  }
}
