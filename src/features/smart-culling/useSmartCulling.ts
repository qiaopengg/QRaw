import { create } from 'zustand';
import type { SmartCullingProgress, SmartCullingSuggestions } from './types';

interface SmartCullingState {
  dialogOpen: boolean;
  isRunning: boolean;
  progress: SmartCullingProgress | null;
  suggestions: SmartCullingSuggestions | null;
  error: string | null;
  setSmartCulling: (
    updater: Partial<SmartCullingState> | ((state: SmartCullingState) => Partial<SmartCullingState>),
  ) => void;
}

export const useSmartCullingStore = create<SmartCullingState>((set) => ({
  dialogOpen: false,
  isRunning: false,
  progress: null,
  suggestions: null,
  error: null,
  setSmartCulling: (updater) => set((state) => (typeof updater === 'function' ? updater(state) : updater)),
}));
