import type { ComponentType, KeyboardEvent } from 'react';
import type { Adjustments } from '../utils/adjustments';
import type { RenderSize } from '../hooks/useImageRenderSize';
import type { ImageFile, KeybindHandler, SelectedImage } from '../components/ui/AppProperties';

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

export interface LibraryFeatureContext {
  currentFolderPath: string | null;
  imageList: ImageFile[];
  allImageList?: ImageFile[];
  selectedPaths: string[];
  onLibraryRefresh?(): void | Promise<void>;
}

export type LibraryHeaderActionSlotProps = LibraryFeatureContext;

export interface LibraryFeatureViewSlotProps extends LibraryFeatureContext {
  onBackToLibrary(): void;
}

export interface LibraryThumbnailBadgeSlotProps {
  image: ImageFile;
}

export interface LibraryFeatureFilterPredicateContext {
  image: ImageFile;
  imageRatings: Record<string, number>;
}

export interface LibraryFeatureFilterOption {
  value: string;
  label: string;
  predicate(context: LibraryFeatureFilterPredicateContext): boolean;
}

export interface LibraryFeatureFilterGroup {
  key: string;
  label: string;
  options: LibraryFeatureFilterOption[];
}

export interface LibraryFeatureSlots {
  filterGroups?: LibraryFeatureFilterGroup[];
  headerActions?: Array<ComponentType<LibraryHeaderActionSlotProps>>;
  thumbnailBadges?: Array<ComponentType<LibraryThumbnailBadgeSlotProps>>;
  views?: Record<string, ComponentType<LibraryFeatureViewSlotProps>>;
}

export interface AppFeatureRegistration {
  editor?: EditorFeatureSlots;
  library?: LibraryFeatureSlots;
  keyboardActions?: Record<string, KeybindHandler>;
}
