import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { SMART_CULLING_EVENT } from './constants';
import type { SmartCullingSnapshot } from './types';
import { screenForTaskTransition } from './navigation';
import { useSmartCullingStore } from './useSmartCulling';

export function useSmartCullingEvents() {
  useEffect(() => {
    let active = true;
    const unlisten = listen<SmartCullingSnapshot>(SMART_CULLING_EVENT, ({ payload }) => {
      if (!active) return;
      const current = useSmartCullingStore.getState();
      const nextScreen = screenForTaskTransition(current.screen, payload);
      const clearPeople = ['readyForReview', 'completed', 'unsupported', 'failed'].includes(payload.state);
      current.setState({ snapshot: payload, screen: nextScreen, ...(clearPeople ? { keyPeople: [] } : {}) });
    });
    return () => {
      active = false;
      void unlisten.then((dispose) => dispose());
    };
  }, []);
}
