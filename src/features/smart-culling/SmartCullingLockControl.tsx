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
    const selectedImages = images.filter((image) => !image.is_virtual_copy && selected.has(image.path));
    const selectedGroupIds = new Set(selectedImages.flatMap((image) => (image.group_id ? [image.group_id] : [])));
    const assetImages = images.filter(
      (image) =>
        !image.is_virtual_copy &&
        (selected.has(image.path) || (image.group_id && selectedGroupIds.has(image.group_id))),
    );
    const records = assetImages.map(getSmartCullingImageMetadata).filter((record) => record !== undefined);
    return {
      paths: selectedImages.map((image) => image.path),
      manageable: selectedImages.length > 0 && records.length > 0,
      allLocked: records.length > 0 && records.every((record) => record.locked),
    };
  }, [images, selectedPaths]);

  if (!selection.manageable) return null;
  const nextLocked = !selection.allLocked;
  const label = selection.allLocked ? tx('unlockSelection') : tx('lockSelection');
  const hint = selection.allLocked ? tx('unlockSelectionHint') : tx('lockSelectionHint');

  const changeLock = async () => {
    setBusy(true);
    try {
      await runSmartCullingCommand({ action: 'setLock', paths: selection.paths, locked: nextLocked }, true);
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
    <Button
      className="h-12 w-12 bg-transparent text-text-primary shadow-none p-0 flex items-center justify-center"
      onClick={() => void changeLock()}
      disabled={busy}
      data-tooltip={`${label}：${hint}`}
      aria-label={label}
    >
      {busy ? (
        <Loader2 className="animate-spin" size={20} />
      ) : selection.allLocked ? (
        <LockOpen size={20} />
      ) : (
        <LockKeyhole size={20} />
      )}
    </Button>
  );
}
