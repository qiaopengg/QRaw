import type { KeybindHandler, SelectedImage } from '../../components/ui/AppProperties';
import type { FocusRegion } from './types';

export interface FocusAreasController {
  focusRegions: FocusRegion[];
  showFocusAreas: boolean;
  toggleFocusAreas(): void;
}

export function createFocusAreaShortcutActions(
  selectedImage: SelectedImage | null,
  toggleFocusAreas: () => void,
): Record<string, KeybindHandler> {
  return {
    toggle_focus_areas: {
      shouldFire: () => !!selectedImage,
      execute: (event) => {
        event.preventDefault();
        toggleFocusAreas();
      },
    },
  };
}
