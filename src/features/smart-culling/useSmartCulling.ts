import { create } from 'zustand';
import type { SmartCullingProgress, SmartCullingTaskResult } from './types';

interface SmartCullingState {
  activeTaskId: string | null;
  dialogOpen: boolean;
  error: string | null;
  isRunning: boolean;
  progress: SmartCullingProgress | null;
  result: SmartCullingTaskResult | null;
  setSmartCulling: (
    updater: Partial<SmartCullingState> | ((state: SmartCullingState) => Partial<SmartCullingState>),
  ) => void;
}

export const useSmartCullingStore = create<SmartCullingState>((set) => ({
  activeTaskId: null,
  dialogOpen: false,
  error: null,
  isRunning: false,
  progress: null,
  result: null,
  setSmartCulling: (updater) => set((state) => (typeof updater === 'function' ? updater(state) : updater)),
}));
