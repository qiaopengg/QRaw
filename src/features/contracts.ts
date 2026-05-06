import type { ComponentType, KeyboardEvent } from 'react';
import type { Adjustments } from '../utils/adjustments';
import type { RenderSize } from '../hooks/useImageRenderSize';
import type { KeybindHandler, SelectedImage } from '../components/ui/AppProperties';

export interface AppFeatureKeyboardContext {
  selectedImage: SelectedImage | null;
}

export interface EditorToolbarFeatureSlotProps {
  onKeyDown(event: KeyboardEvent<HTMLButtonElement>): void;
}

export interface ImageCanvasFeatureSlotProps {
  adjustments: Adjustments;
  effectiveCursor: string;
  imageRenderSize: RenderSize;
  imageSize: { width: number; height: number };
  isShowingOriginal: boolean;
}

export interface EditorFeatureSlots {
  imageCanvasOverlays?: Array<ComponentType<ImageCanvasFeatureSlotProps>>;
  toolbarControls?: Array<ComponentType<EditorToolbarFeatureSlotProps>>;
}

export interface AppFeatureRegistration {
  editor?: EditorFeatureSlots;
  keyboardActions?: Record<string, KeybindHandler>;
}
