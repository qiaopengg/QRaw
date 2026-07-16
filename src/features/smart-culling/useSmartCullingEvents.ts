import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { toast } from 'react-toastify';
import { SMART_CULLING_EVENT } from './constants';
import { useSmartCullingText } from './i18n';
import type { SmartCullingSnapshot } from './types';
import { screenForSnapshot, useSmartCullingStore } from './useSmartCulling';

export function useSmartCullingEvents() {
  const tx = useSmartCullingText();
  const previousState = useRef<string | null>(null);
  useEffect(() => {
    let active = true;
    const unlisten = listen<SmartCullingSnapshot>(SMART_CULLING_EVENT, ({ payload }) => {
      if (!active) return;
      const current = useSmartCullingStore.getState();
      const nextScreen =
        payload.state === 'readyForReview' && current.screen === 'review' ? 'review' : screenForSnapshot(payload);
      const clearPeople = ['readyForReview', 'completed', 'unsupported', 'failed'].includes(payload.state);
      current.setState({ snapshot: payload, screen: nextScreen, ...(clearPeople ? { keyPeople: [] } : {}) });
      if (payload.state === 'readyForReview' && previousState.current !== 'readyForReview') {
        toast.info(payload.progress.partial ? tx('partialFinishedNotice') : tx('analysisFinishedNotice'));
      }
      previousState.current = payload.state;
    });
    return () => {
      active = false;
      void unlisten.then((dispose) => dispose());
    };
  }, [tx]);
}
