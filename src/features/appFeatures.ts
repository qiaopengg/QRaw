import type { AppFeatureRegistration, AppFeatureKeyboardContext } from './contracts';
import { useFocusAreaFeature } from './focus-areas/feature';

export function useAppFeatures(context: AppFeatureKeyboardContext): AppFeatureRegistration {
  const registrations = [useFocusAreaFeature(context)];

  return {
    editor: {
      imageCanvasOverlays: registrations.flatMap((feature) => feature.editor?.imageCanvasOverlays ?? []),
      toolbarControls: registrations.flatMap((feature) => feature.editor?.toolbarControls ?? []),
    },
    keyboardActions: Object.assign({}, ...registrations.map((feature) => feature.keyboardActions ?? {})),
  };
}
