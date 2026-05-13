import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useUIStore } from '../../store/useUIStore';
import { SMART_CULLING_REVIEW_VIEW } from './constants';
import { useSmartCullingStore } from './useSmartCulling';
import type { SmartCullingProgress, SmartCullingTaskResult } from './types';

export function useSmartCullingEvents() {
  const setSmartCulling = useSmartCullingStore((state) => state.setSmartCulling);

  useEffect(() => {
    let active = true;
    const unlisten = Promise.all([
      listen<SmartCullingProgress>('smart-culling:progress', (event) => {
        if (!active) return;
        setSmartCulling({ activeTaskId: event.payload.taskId, progress: event.payload, isRunning: true, error: null });
      }),
      listen<SmartCullingTaskResult>('smart-culling:review-ready', (event) => {
        if (!active) return;
        setSmartCulling({
          activeTaskId: event.payload.taskId,
          progress: null,
          result: event.payload,
          isRunning: false,
          error: null,
        });
        useUIStore.getState().setUI({ activeView: SMART_CULLING_REVIEW_VIEW });
      }),
      listen<{ taskId: string; error: string }>('smart-culling:failed', (event) => {
        if (!active) return;
        setSmartCulling({
          activeTaskId: event.payload.taskId,
          progress: null,
          isRunning: false,
          error: event.payload.error,
        });
      }),
      listen<{ taskId: string }>('smart-culling:cancelled', (event) => {
        if (!active) return;
        setSmartCulling((state) => ({
          activeTaskId: event.payload.taskId,
          progress: null,
          isRunning: false,
          result: state.activeTaskId === event.payload.taskId ? null : state.result,
          error: '智能选图任务已取消。',
        }));
      }),
    ]);

    return () => {
      active = false;
      unlisten.then((callbacks) => callbacks.forEach((cb) => cb()));
    };
  }, [setSmartCulling]);
}
