import { Sparkles, Loader2 } from 'lucide-react';
import Button from '../../components/ui/Button';
import type { LibraryHeaderActionSlotProps } from '../contracts';
import { useSmartCullingStore } from './useSmartCulling';
import { useSmartCullingEvents } from './useSmartCullingEvents';
import SmartCullingDialog from './SmartCullingDialog';

export default function SmartCullingEntry(props: LibraryHeaderActionSlotProps) {
  useSmartCullingEvents();
  const { isRunning, progress, setSmartCulling } = useSmartCullingStore();

  return (
    <>
      <Button
        className="h-12 w-12 bg-surface text-text-primary shadow-none p-0 flex items-center justify-center"
        onClick={() => setSmartCulling({ dialogOpen: true })}
        data-tooltip={isRunning ? progress?.stage || '智能选图运行中' : '智能选图'}
      >
        {isRunning ? <Loader2 className="w-8 h-8 animate-spin" /> : <Sparkles className="w-8 h-8" />}
      </Button>
      <SmartCullingDialog {...props} />
    </>
  );
}
