import type { AppFeatureKeyboardContext, AppFeatureRegistration } from '../contracts';
import { createFocusAreaShortcutActions, FocusAreasCanvasEntry, FocusAreasToolbarEntry, useFocusAreas } from './index';

export function useFocusAreaFeature(context: AppFeatureKeyboardContext): AppFeatureRegistration {
  const focusAreas = useFocusAreas(context.selectedImage);

  return {
    editor: {
      imageCanvasOverlays: [(props) => <FocusAreasCanvasEntry {...props} focusAreas={focusAreas} />],
      toolbarControls: [(props) => <FocusAreasToolbarEntry {...props} focusAreas={focusAreas} />],
    },
    keyboardActions: createFocusAreaShortcutActions(context.selectedImage, focusAreas.toggleFocusAreas),
  };
}
