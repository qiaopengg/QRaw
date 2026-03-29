import { useState, useRef, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Invokes } from '../components/ui/AppProperties';

export function useThumbnails() {
  const [loading, setLoading] = useState(false);
  const requestedPathsRef = useRef<Set<string>>(new Set());
  const queueTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingQueueRef = useRef<Set<string>>(new Set());
  const loadingCountRef = useRef(0);

  useEffect(() => {
    let unlistenComplete: (() => void) | undefined;
    const setupListener = async () => {
      unlistenComplete = await listen('thumbnail-generation-complete', () => {
        loadingCountRef.current = Math.max(0, loadingCountRef.current - 1);
        if (loadingCountRef.current === 0) {
          setLoading(false);
        }
      });
    };
    setupListener();
    return () => {
      if (unlistenComplete) unlistenComplete();
    };
  }, []);

  const clearThumbnailQueue = useCallback(() => {
    requestedPathsRef.current.clear();
    pendingQueueRef.current.clear();
    if (queueTimeoutRef.current) {
      clearTimeout(queueTimeoutRef.current);
    }
    loadingCountRef.current = 0;
    setLoading(false);
  }, []);

  const requestThumbnails = useCallback((paths: string[]) => {
    let added = false;
    paths.forEach((p) => {
      if (!requestedPathsRef.current.has(p)) {
        requestedPathsRef.current.add(p);
        pendingQueueRef.current.add(p);
        added = true;
      }
    });

    if (added) {
      if (queueTimeoutRef.current) clearTimeout(queueTimeoutRef.current);
      queueTimeoutRef.current = setTimeout(() => {
        const pathsToRequest = Array.from(pendingQueueRef.current);
        pendingQueueRef.current.clear();

        if (pathsToRequest.length > 0) {
          loadingCountRef.current += 1;
          setLoading(true);
          invoke(Invokes.GenerateThumbnailsProgressive, { paths: pathsToRequest }).catch((err) => {
            console.error('Failed to request thumbnails:', err);
            pathsToRequest.forEach((p) => requestedPathsRef.current.delete(p));
            loadingCountRef.current = Math.max(0, loadingCountRef.current - 1);
            if (loadingCountRef.current === 0) setLoading(false);
          });
        }
      }, 150);
    }
  }, []);

  return { loading, requestThumbnails, clearThumbnailQueue };
}
