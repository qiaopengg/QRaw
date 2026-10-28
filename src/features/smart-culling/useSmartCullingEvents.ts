import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { SMART_CULLING_EVENT } from './constants';
import type { SmartCullingSnapshot } from './types';
import { applySmartCullingEvent } from './useSmartCulling';

export function useSmartCullingEvents() {
  useEffect(() => {
    let active = true;
    const unlisten = listen<SmartCullingSnapshot>(SMART_CULLING_EVENT, ({ payload }) => {
      if (!active) return;
      applySmartCullingEvent(payload);
    });
    return () => {
      active = false;
      void unlisten.then((dispose) => dispose());
    };
  }, []);
}
