import { Target } from 'lucide-react';
import clsx from 'clsx';

interface FocusAreaToolbarButtonProps {
  onKeyDown(event: React.KeyboardEvent<HTMLButtonElement>): void;
  onToggleFocusAreas(): void;
  showFocusAreas: boolean;
}

export default function FocusAreaToolbarButton({
  onKeyDown,
  onToggleFocusAreas,
  showFocusAreas,
}: FocusAreaToolbarButtonProps) {
  return (
    <button
      className={clsx(
        'p-2 rounded-full transition-colors',
        showFocusAreas
          ? 'bg-accent text-button-text hover:bg-accent/90'
          : 'bg-surface hover:bg-card-active text-text-primary',
      )}
      onClick={onToggleFocusAreas}
      onKeyDown={onKeyDown}
      data-tooltip={showFocusAreas ? '隐藏对焦区域 (Shift+F)' : '显示对焦区域 (Shift+F)'}
    >
      <Target size={20} />
    </button>
  );
}
