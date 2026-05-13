import type { AppFeatureRegistration, AppFeatureKeyboardContext } from './contracts';
import { useFocusAreaFeature } from './focus-areas/feature';
import { useSmartCullingFeature } from './smart-culling/feature';

export function useAppFeatures(context: AppFeatureKeyboardContext): AppFeatureRegistration {
  const registrations = [useFocusAreaFeature(context), useSmartCullingFeature()];

  return {
    editor: {
      imageCanvasOverlays: registrations.flatMap((feature) => feature.editor?.imageCanvasOverlays ?? []),
      toolbarControls: registrations.flatMap((feature) => feature.editor?.toolbarControls ?? []),
    },
    library: {
      filterGroups: registrations.flatMap((feature) => feature.library?.filterGroups ?? []),
      headerActions: registrations.flatMap((feature) => feature.library?.headerActions ?? []),
      thumbnailBadges: registrations.flatMap((feature) => feature.library?.thumbnailBadges ?? []),
      views: Object.assign({}, ...registrations.map((feature) => feature.library?.views ?? {})),
    },
    keyboardActions: Object.assign({}, ...registrations.map((feature) => feature.keyboardActions ?? {})),
  };
}
