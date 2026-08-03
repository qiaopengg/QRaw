import { useEffect, useRef } from 'react';
import { Loader2, Sparkles } from 'lucide-react';
import Button from '../../components/ui/Button';
import { toast } from 'react-toastify';
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
  const reconciled = useRef('');
  const running =
    snapshot && ['indexing', 'rendering', 'analyzing', 'organizing', 'cancelling'].includes(snapshot.state);
  const pending = snapshot?.state === 'readyForReview';

  useEffect(() => {
    void runSmartCullingCommand({ action: 'status' }).catch(() => undefined);
  }, []);
  useEffect(() => {
    const stale = (allImageList ?? imageList)
      .filter((image) => !image.is_virtual_copy && needsManualOwnershipReconciliation(image))
      .map((image) => image.path);
    const key = stale.join('\n');
    if (!key) {
      reconciled.current = '';
      return;
    }
    if (key === reconciled.current) return;
    reconciled.current = key;
    void runSmartCullingCommand({ action: 'reconcileManual', paths: stale }, true)
      .then(() => onLibraryRefresh?.())
      .catch(() => {
        toast.error(tx('manualSyncFailed'));
        return onLibraryRefresh?.();
      });
  }, [allImageList, imageList, onLibraryRefresh, tx]);

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
