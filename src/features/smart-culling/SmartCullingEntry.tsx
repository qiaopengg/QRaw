import { Sparkles, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import Button from '../../components/ui/Button';
import type { LibraryHeaderActionSlotProps } from '../contracts';
import { useSmartCullingStore } from './useSmartCulling';
import SmartCullingDialog from './SmartCullingDialog';

export default function SmartCullingEntry({ imageList, allImageList }: LibraryHeaderActionSlotProps) {
  const { t } = useTranslation();
  const { isRunning, progress, setSmartCulling } = useSmartCullingStore();
  const paths = (allImageList ?? imageList).map((image) => image.path);

  return (
    <>
      <Button
        className="h-12 w-12 bg-surface text-text-primary shadow-none p-0 flex items-center justify-center"
        onClick={() => setSmartCulling({ dialogOpen: true })}
        data-tooltip={isRunning ? progress?.stage || t('modals.smartCulling.title') : t('modals.smartCulling.title')}
      >
        {isRunning ? <Loader2 className="w-8 h-8 animate-spin" /> : <Sparkles className="w-8 h-8" />}
      </Button>
      <SmartCullingDialog imagePaths={paths} />
    </>
  );
}
