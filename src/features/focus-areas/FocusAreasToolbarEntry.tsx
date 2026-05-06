import type { KeyboardEvent } from 'react';
import type { FocusAreasController } from './contracts';
import FocusAreaToolbarButton from './FocusAreaToolbarButton';

interface FocusAreasToolbarEntryProps {
  focusAreas: FocusAreasController;
  onKeyDown(event: KeyboardEvent<HTMLButtonElement>): void;
}

export default function FocusAreasToolbarEntry({
  focusAreas,
  onKeyDown,
}: FocusAreasToolbarEntryProps) {
  return (
    <FocusAreaToolbarButton
      onKeyDown={onKeyDown}
      onToggleFocusAreas={focusAreas.toggleFocusAreas}
      showFocusAreas={focusAreas.showFocusAreas}
    />
  );
}
