import type { ImageFile } from '../../components/ui/AppProperties';

export interface SmartCullingImageMetadata {
  degraded?: boolean;
  groupId?: string | null;
  groupRank?: number | null;
  groupSize?: number | null;
  rating?: number;
  colorLabel?: string | null;
  reasonCodes?: string[];
  reasonText?: string;
  status?: string;
  taskId?: string;
}

export function getSmartCullingImageMetadata(image: ImageFile): SmartCullingImageMetadata | undefined {
  const value = image.featureData?.smartCulling;
  return value && typeof value === 'object' ? (value as SmartCullingImageMetadata) : undefined;
}
