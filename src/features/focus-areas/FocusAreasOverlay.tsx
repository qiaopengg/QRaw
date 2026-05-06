import { Stage, Layer, Rect } from 'react-konva';
import type { Adjustments } from '../../utils/adjustments';
import type { RenderSize } from '../../hooks/useImageRenderSize';
import type { FocusRegion } from './types';

interface FocusAreasOverlayProps {
  adjustments: Adjustments;
  effectiveCursor: string;
  focusRegions: FocusRegion[];
  imageRenderSize: RenderSize;
  imageSize: { width: number; height: number };
  isShowingOriginal: boolean;
  showFocusAreas: boolean;
}

interface TransformedFocusPoint {
  rotation: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

function transformFocusPoint(
  region: FocusRegion,
  adjustments: Adjustments,
  imageSize: { width: number; height: number },
  scale: number,
): TransformedFocusPoint {
  let imgW = imageSize.width;
  let imgH = imageSize.height;

  let x = region.x * imgW;
  let y = region.y * imgH;
  let w = region.width * imgW;
  let h = region.height * imgH;

  const orientationSteps = adjustments.orientationSteps || 0;
  if (orientationSteps > 0) {
    for (let i = 0; i < orientationSteps; i++) {
      const temp = x;
      x = imgH - y - h;
      y = temp;
      [w, h] = [h, w];
      [imgW, imgH] = [imgH, imgW];
    }
  }

  if (adjustments.flipHorizontal) {
    x = imgW - x - w;
  }

  if (adjustments.flipVertical) {
    y = imgH - y - h;
  }

  const rotation = adjustments.rotation || 0;
  if (rotation !== 0) {
    const angle = (rotation * Math.PI) / 180;
    const cx = imgW / 2;
    const cy = imgH / 2;
    const cos = Math.cos(angle);
    const sin = Math.sin(angle);
    const dx = x + w / 2 - cx;
    const dy = y + h / 2 - cy;
    x = cx + dx * cos - dy * sin - w / 2;
    y = cy + dx * sin + dy * cos - h / 2;
  }

  const crop = adjustments.crop;
  if (crop) {
    const isPercent = crop.unit === '%';
    const cropX = isPercent ? ((crop.x ?? 0) / 100) * imgW : (crop.x ?? 0);
    const cropY = isPercent ? ((crop.y ?? 0) / 100) * imgH : (crop.y ?? 0);
    x -= cropX;
    y -= cropY;
  }

  const safeScale = scale > 0 ? scale : 1.0;

  return {
    rotation,
    x: x * safeScale,
    y: y * safeScale,
    width: w * safeScale,
    height: h * safeScale,
  };
}

function getFocusRegionStroke(region: FocusRegion): string {
  if (region.kind === 'eye') {
    return '#3b82f6';
  }
  if (region.kind === 'face') {
    return '#22c55e';
  }
  if (region.is_primary) {
    return '#ef4444';
  }
  return '#f97316';
}

function getFocusRegionDash(region: FocusRegion): number[] | undefined {
  return region.kind === 'point' ? [6, 4] : undefined;
}

export default function FocusAreasOverlay({
  adjustments,
  effectiveCursor,
  focusRegions,
  imageRenderSize,
  imageSize,
  isShowingOriginal,
  showFocusAreas,
}: FocusAreasOverlayProps) {
  if (!showFocusAreas || focusRegions.length === 0) {
    return null;
  }

  return (
    <Stage
      height={imageRenderSize.height}
      style={{
        cursor: effectiveCursor,
        left: `${imageRenderSize.offsetX}px`,
        opacity: isShowingOriginal ? 0 : 1,
        transition: 'opacity 150ms ease-in-out',
        position: 'absolute',
        top: `${imageRenderSize.offsetY}px`,
        zIndex: 3,
        pointerEvents: 'none',
      }}
      width={imageRenderSize.width}
    >
      <Layer>
        {focusRegions.map((region, index) => {
          const transformed = transformFocusPoint(region, adjustments, imageSize, imageRenderSize.scale);
          const centerX = transformed.x + transformed.width / 2;
          const centerY = transformed.y + transformed.height / 2;
          const minSize = region.kind === 'point' ? 28 : 18;
          const drawWidth = Math.max(transformed.width, minSize);
          const drawHeight = Math.max(transformed.height, minSize);
          const stroke = getFocusRegionStroke(region);

          return (
            <Rect
              key={`focus-${index}`}
              x={centerX}
              y={centerY}
              width={drawWidth}
              height={drawHeight}
              offsetX={drawWidth / 2}
              offsetY={drawHeight / 2}
              rotation={transformed.rotation}
              stroke={stroke}
              strokeWidth={region.is_primary ? 2.5 : 2}
              dash={getFocusRegionDash(region)}
              listening={false}
              shadowColor="black"
              shadowBlur={4}
              shadowOpacity={0.6}
            />
          );
        })}
      </Layer>
    </Stage>
  );
}
