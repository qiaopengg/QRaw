import { Loader2, LockKeyhole, LockOpen } from 'lucide-react';
import { useMemo, useState } from 'react';
import { toast } from 'react-toastify';
import Button from '../../components/ui/Button';
import type { LibraryHeaderActionSlotProps } from '../contracts';
import { useSmartCullingText } from './i18n';
import { getSmartCullingImageMetadata } from './metadata';
import { runSmartCullingCommand } from './useSmartCulling';

export default function SmartCullingLockControl({
  imageList,
  allImageList,
  selectedPaths,
  onLibraryRefresh,
}: LibraryHeaderActionSlotProps) {
  const tx = useSmartCullingText();
  const [busy, setBusy] = useState(false);
  const images = allImageList ?? imageList;
  const selection = useMemo(() => {
    const selected = new Set(selectedPaths);
    const assetKey = (image: (typeof images)[number]) => image.group_id || image.path;
    const selectedEntries = images.filter((image) => selected.has(image.path));
    const selectedAssetKeys = new Set(selectedEntries.map(assetKey));
    const selectedImages = images.filter((image) => !image.is_virtual_copy && selectedAssetKeys.has(assetKey(image)));
    const selectedAssets = [...selectedAssetKeys].map((key) =>
      images.filter((image) => !image.is_virtual_copy && assetKey(image) === key),
    );
    const allLocked =
      selectedAssets.length > 0 &&
      selectedAssets.every(
        (members) =>
          members.length > 0 && members.every((image) => getSmartCullingImageMetadata(image)?.locked === true),
      );
    const anyLocked = selectedAssets.some((members) =>
      members.some((image) => getSmartCullingImageMetadata(image)?.locked === true),
    );
    return {
      paths: selectedImages.map((image) => image.path),
      manageable: selectedImages.length > 0,
      mappedVirtual: selectedEntries.some((image) => image.is_virtual_copy),
      unresolvedVirtual: selectedEntries.some((image) => image.is_virtual_copy) && selectedImages.length === 0,
      state: allLocked ? ('locked' as const) : anyLocked ? ('mixed' as const) : ('unlocked' as const),
    };
  }, [images, selectedPaths]);

  if (!selection.manageable) {
    if (!selection.unresolvedVirtual) return null;
    return (
      <div className="sc-lock-control" role="status">
        <span className="sc-lock-copy">
          <strong>{tx('virtualCopy')}</strong>
          <small>{tx('virtualCopyLockUnavailable')}</small>
        </span>
      </div>
    );
  }
  const label =
    selection.state === 'mixed'
      ? tx('mixedLockState')
      : selection.state === 'locked'
        ? tx('lockedResult')
        : tx('unlockedSelectionState');
  const hint = selection.mappedVirtual
    ? tx('virtualCopyMapped')
    : selection.state === 'mixed'
      ? tx('chooseBatchLockState')
      : selection.state === 'locked'
        ? tx('unlockSelectionHint')
        : tx('lockSelectionHint');

  const changeLock = async (locked: boolean) => {
    setBusy(true);
    try {
      const snapshot = await runSmartCullingCommand({ action: 'setLock', paths: selection.paths, locked }, true);
      const summary = snapshot.lockChangeSummary;
      if (summary?.failed) {
        toast.error(
          <div className="sc-lock-failure-toast">
            <strong>
              {tx('lockPartialResult')} {summary.succeeded + summary.unchanged} / {summary.attempted}
            </strong>
            {summary.failures.map((failure) => (
              <span key={`${failure.path}:${failure.detail}`}>
                {failure.path}: {failure.detail}
              </span>
            ))}
          </div>,
          { autoClose: false },
        );
      } else {
        toast.success(tx(locked ? 'lockApplied' : 'unlockApplied'));
      }
    } catch {
      toast.error(tx('lockChangeFailed'));
    } finally {
      try {
        await onLibraryRefresh?.();
      } finally {
        setBusy(false);
      }
    }
  };

  return (
    <div className="sc-lock-control" role="group" aria-label={tx('lockSelection')}>
      <span className="sc-lock-copy">
        <strong>{label}</strong>
        <small>{hint}</small>
      </span>
      <span className="sc-lock-actions">
        {selection.state !== 'locked' ? (
          <Button
            className="sc-lock-action"
            onClick={() => void changeLock(true)}
            disabled={busy}
            data-tooltip={tx('lockSelection')}
            aria-label={tx('lockSelection')}
          >
            {busy ? <Loader2 className="animate-spin" size={18} /> : <LockKeyhole size={18} />}
          </Button>
        ) : null}
        {selection.state !== 'unlocked' ? (
          <Button
            className="sc-lock-action"
            onClick={() => void changeLock(false)}
            disabled={busy}
            data-tooltip={tx('unlockSelection')}
            aria-label={tx('unlockSelection')}
          >
            {busy ? <Loader2 className="animate-spin" size={18} /> : <LockOpen size={18} />}
          </Button>
        ) : null}
      </span>
    </div>
  );
}
