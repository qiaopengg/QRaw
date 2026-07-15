import type { ImageFile } from '../../components/ui/AppProperties';

export interface SmartCullingImageMetadata {
  score?: number;
  reasonText?: string;
  status?: string;
}

export function getSmartCullingImageMetadata(image: ImageFile): SmartCullingImageMetadata | undefined {
  const value = image.featureData?.smartCulling;
  return value && typeof value === 'object' ? (value as SmartCullingImageMetadata) : undefined;
}
