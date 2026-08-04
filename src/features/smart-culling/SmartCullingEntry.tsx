import { useEffect, useRef } from 'react';
import { Loader2, Sparkles } from 'lucide-react';
import Button from '../../components/ui/Button';
import { toast } from 'react-toastify';
import { useLibraryStore } from '../../store/useLibraryStore';
import { useUIStore } from '../../store/useUIStore';
import type { LibraryHeaderActionSlotProps } from '../contracts';
import { SMART_CULLING_VIEW } from './constants';
import { useSmartCullingText } from './i18n';
import { needsManualOwnershipReconciliation } from './metadata';
import { SmartCullingReadyNotice } from './components/SmartCullingReadyNotice';
import { runSmartCullingCommand, useSmartCullingStore } from './useSmartCulling';
import { useSmartCullingEvents } from './useSmartCullingEvents';

export default function SmartCullingEntry({
  currentFolderPath,
  imageList,
  allImageList,
  onLibraryRefresh,
}: LibraryHeaderActionSlotProps) {
  useSmartCullingEvents();
  const tx = useSmartCullingText();
  const { snapshot, setState: setSmartCullingState } = useSmartCullingStore();
  const setUI = useUIStore((state) => state.setUI);
  const imageRatings = useLibraryStore((state) => state.imageRatings);
  const reconciled = useRef('');
  const running =
    snapshot && ['indexing', 'rendering', 'analyzing', 'organizing', 'cancelling'].includes(snapshot.state);
  const pending = snapshot?.state === 'readyForReview';

  useEffect(() => {
    void runSmartCullingCommand({ action: 'status' }).catch(() => undefined);
  }, []);
  useEffect(() => {
    const stale = (allImageList ?? imageList)
      .filter(
        (image) =>
          !image.is_virtual_copy && needsManualOwnershipReconciliation(image, imageRatings[image.path] ?? image.rating),
      )
      .map((image) => image.path);
    const key = stale.join('\n');
    if (!key) {
      reconciled.current = '';
      if (useSmartCullingStore.getState().manualSyncPending) {
        setSmartCullingState({ manualSyncPending: false });
      }
      return;
    }
    if (key === reconciled.current) return;
    setSmartCullingState({ manualSyncPending: true });
    let cancelled = false;
    const reconcile = async () => {
      for (const delay of [0, 100, 300]) {
        if (delay) await new Promise((resolve) => window.setTimeout(resolve, delay));
        if (cancelled) return;
        try {
          await runSmartCullingCommand({ action: 'reconcileManual', paths: stale }, true);
          if (cancelled) return;
          reconciled.current = key;
          setSmartCullingState({ manualSyncPending: false });
          await onLibraryRefresh?.();
          return;
        } catch {
          // The host rating write and provenance reconciliation are separate
          // commands. Retry only while waiting for the visible rating to land.
        }
      }
      if (!cancelled) {
        toast.error(tx('manualSyncFailed'));
        await onLibraryRefresh?.();
      }
    };
    void reconcile();
    return () => {
      cancelled = true;
    };
  }, [allImageList, imageList, imageRatings, onLibraryRefresh, setSmartCullingState, tx]);

  const open = async () => {
    if (pending) {
      setSmartCullingState({ screen: 'review' });
      setUI({ activeView: SMART_CULLING_VIEW });
      return;
    }
    if (running || snapshot?.state === 'completed') {
      setUI({ activeView: SMART_CULLING_VIEW });
      return;
    }
    if (!currentFolderPath) return;
    const inspection = runSmartCullingCommand({ action: 'inspect', rootPath: currentFolderPath });
    setUI({ activeView: SMART_CULLING_VIEW });
    await inspection.catch(() => undefined);
  };

  return (
    <>
      <Button
        className="sc-entry-button"
        onClick={() => void open()}
        data-tooltip={tx('title')}
        aria-label={tx('title')}
      >
        {running ? <Loader2 className="animate-spin" size={20} /> : <Sparkles size={20} />}
        {pending ? <span className="sc-entry-dot" /> : null}
      </Button>
      <SmartCullingReadyNotice
        snapshot={snapshot}
        onOpenReview={() => {
          setSmartCullingState({ screen: 'review' });
          setUI({ activeView: SMART_CULLING_VIEW });
        }}
      />
    </>
  );
}
