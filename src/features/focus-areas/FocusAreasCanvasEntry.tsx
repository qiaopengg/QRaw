import type { Adjustments } from '../../utils/adjustments';
import type { RenderSize } from '../../hooks/useImageRenderSize';
import type { FocusAreasController } from './contracts';
import FocusAreasOverlay from './FocusAreasOverlay';

interface FocusAreasCanvasEntryProps {
  adjustments: Adjustments;
  effectiveCursor: string;
  focusAreas: FocusAreasController;
  imageRenderSize: RenderSize;
  imageSize: { width: number; height: number };
  isShowingOriginal: boolean;
}

export default function FocusAreasCanvasEntry({
  adjustments,
  effectiveCursor,
  focusAreas,
  imageRenderSize,
  imageSize,
  isShowingOriginal,
}: FocusAreasCanvasEntryProps) {
  return (
    <FocusAreasOverlay
      adjustments={adjustments}
      effectiveCursor={effectiveCursor}
      focusRegions={focusAreas.focusRegions}
      imageRenderSize={imageRenderSize}
      imageSize={imageSize}
      isShowingOriginal={isShowingOriginal}
      showFocusAreas={focusAreas.showFocusAreas}
    />
  );
}
