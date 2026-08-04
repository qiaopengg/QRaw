import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';
import { Invokes } from '../../../components/ui/AppProperties';

interface CachedPreview {
  url: string;
  lastUsed: number;
  users: number;
}

const MAX_UNUSED_PREVIEWS = 4;
const previewCache = new Map<string, CachedPreview>();
const pendingPreviews = new Map<string, Promise<CachedPreview>>();

function previewKey(path: string, thumbKey: string, retryVersion = 0) {
  return `${path}\u0000${thumbKey}\u0000${retryVersion}`;
}

function pruneUnusedPreviews() {
  const unused = [...previewCache.entries()]
    .filter(([, preview]) => preview.users === 0)
    .sort((left, right) => left[1].lastUsed - right[1].lastUsed);

  while (unused.length > MAX_UNUSED_PREVIEWS) {
    const [key, preview] = unused.shift()!;
    URL.revokeObjectURL(preview.url);
    previewCache.delete(key);
  }
}

function retainPreview(key: string, preview: CachedPreview) {
  preview.users += 1;
  preview.lastUsed = Date.now();
  previewCache.set(key, preview);
}

function releasePreview(key: string) {
  const preview = previewCache.get(key);
  if (!preview) return;
  preview.users = Math.max(0, preview.users - 1);
  preview.lastUsed = Date.now();
  pruneUnusedPreviews();
}

function requestRenderedPreview(path: string, thumbKey: string, retryVersion: number) {
  const pendingKey = previewKey(path, thumbKey, retryVersion);
  const cached = previewCache.get(pendingKey);
  if (cached) return Promise.resolve(cached);

  const pending = pendingPreviews.get(pendingKey);
  if (pending) return pending;

  const request = (async () => {
    const metadata = await invoke<{ adjustments?: unknown } | null>(Invokes.LoadMetadata, { path });
    const adjustments = metadata?.adjustments && typeof metadata.adjustments === 'object' ? metadata.adjustments : {};
    const bytes = await invoke<Uint8Array>(Invokes.GeneratePreviewForPath, {
      path,
      jsAdjustments: adjustments,
    });
    const url = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: 'image/jpeg' }));
    const preview = { url, lastUsed: Date.now(), users: 0 };
    previewCache.set(pendingKey, preview);
    return preview;
  })();
  pendingPreviews.set(pendingKey, request);
  const clearPending = () => {
    if (pendingPreviews.get(pendingKey) === request) pendingPreviews.delete(pendingKey);
  };
  void request.then(clearPending, clearPending);
  return request;
}

export function useRenderedPreview(path: string, fallbackUrl?: string) {
  const [loadedUrl, setLoadedUrl] = useState<string | null>(null);
  const [loading, setLoading] = useState(Boolean(path));
  const [error, setError] = useState<string | null>(null);
  const [retryVersion, setRetryVersion] = useState(0);

  useEffect(() => {
    if (!path) {
      setLoadedUrl(null);
      setLoading(false);
      setError(null);
      return;
    }
    const thumbKey = fallbackUrl ?? '';
    const key = previewKey(path, thumbKey, retryVersion);

    let active = true;
    let retained = false;
    setLoadedUrl(null);
    setLoading(true);
    setError(null);
    const load = async () => {
      try {
        const preview = await requestRenderedPreview(path, thumbKey, retryVersion);
        if (!active) {
          pruneUnusedPreviews();
          return;
        }
        retainPreview(key, preview);
        pruneUnusedPreviews();
        retained = true;
        setLoadedUrl(preview.url);
      } catch (error) {
        console.error('Could not load the current-render smart-culling preview:', error);
        if (active) setError(String(error));
      } finally {
        if (active) setLoading(false);
      }
    };
    void load();
    return () => {
      active = false;
      if (retained) releasePreview(key);
    };
  }, [fallbackUrl, path, retryVersion]);

  return {
    loadedUrl,
    loading,
    error,
    retry: () => setRetryVersion((current) => current + 1),
  };
}
