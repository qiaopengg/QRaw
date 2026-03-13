declare module 'react-image-crop' {
  import { Component, CSSProperties } from 'react';

  export interface Crop {
    x: number;
    y: number;
    width: number;
    height: number;
    unit?: 'px' | '%';
    aspect?: number;
  }

  export interface PercentCrop extends Crop {
    unit: '%';
  }

  export interface PixelCrop extends Crop {
    unit: 'px';
  }

  export interface ReactCropProps {
    crop?: Crop | null;
    onChange: (crop: Crop, percentCrop: PercentCrop) => void;
    onComplete?: (crop: Crop, percentCrop: PercentCrop) => void;
    onDragStart?: (e: MouseEvent) => void;
    onDragEnd?: (e: MouseEvent) => void;
    disabled?: boolean;
    locked?: boolean;
    className?: string;
    style?: CSSProperties;
    children?: React.ReactNode;
    aspect?: number | null;
    minWidth?: number;
    minHeight?: number;
    maxWidth?: number;
    maxHeight?: number;
    keepSelection?: boolean;
    ruleOfThirds?: boolean;
    circularCrop?: boolean;
    renderSelectionAddon?: (state: Record<string, unknown>) => React.ReactNode;
    [key: string]: unknown;
  }

  export default class ReactCrop extends Component<ReactCropProps> {}

  export function makeAspectCrop(crop: Partial<Crop>, aspect: number, mediaWidth: number, mediaHeight: number): Crop;
  export function centerCrop(crop: Crop, mediaWidth: number, mediaHeight: number): Crop;
  export function convertToPercentCrop(crop: Crop, mediaWidth: number, mediaHeight: number): PercentCrop;
  export function convertToPixelCrop(crop: Crop, mediaWidth: number, mediaHeight: number): PixelCrop;
}
