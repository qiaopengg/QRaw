import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useSmartCullingStore } from './useSmartCulling';
import type { SmartCullingProgress, SmartCullingSuggestions } from './types';

export function useSmartCullingEvents() {
  const setSmartCulling = useSmartCullingStore((state) => state.setSmartCulling);

  useEffect(() => {
    let active = true;
    const unlisten = Promise.all([
      listen<number>('smart-culling-start', (event) => {
        if (!active) return;
        setSmartCulling({
          isRunning: true,
          progress: { current: 0, total: event.payload, stage: 'Initializing...' },
          suggestions: null,
          error: null,
        });
      }),
      listen<SmartCullingProgress>('smart-culling-progress', (event) => {
        if (!active) return;
        setSmartCulling({ progress: event.payload });
      }),
      listen<SmartCullingSuggestions>('smart-culling-complete', (event) => {
        if (!active) return;
        setSmartCulling({ isRunning: false, progress: null, suggestions: event.payload });
      }),
    ]);

    return () => {
      active = false;
      unlisten.then((callbacks) => callbacks.forEach((cb) => cb()));
    };
  }, [setSmartCulling]);
}
