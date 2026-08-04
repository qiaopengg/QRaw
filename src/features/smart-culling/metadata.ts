import type { ImageFile } from '../../components/ui/AppProperties';

export interface SmartCullingImageMetadata {
  source: 'ai' | 'manual';
  locked: boolean;
  rating: number;
  colorLabel: 'green' | 'yellow' | 'red' | 'blue' | 'purple' | null;
  reasonCodes?: string[];
  confidence?: number;
  mode?: string;
  modelVersion?: string;
  reasonText?: string;
}

function imageColor(image: ImageFile) {
  return image.tags?.find((tag) => tag.startsWith('color:'))?.slice(6) ?? null;
}

export function getSmartCullingImageMetadata(image: ImageFile): SmartCullingImageMetadata | undefined {
  const value = image.featureData?.smartCullingV2;
  if (!value || typeof value !== 'object') {
    const colorLabel = imageColor(image) as SmartCullingImageMetadata['colorLabel'];
    if (image.rating === 0 && colorLabel === null) return undefined;
    return { source: 'manual', locked: true, rating: image.rating, colorLabel };
  }
  const stored = value as Omit<SmartCullingImageMetadata, 'locked'> & { locked?: boolean };
  const source = stored.source === 'ai' || stored.source === 'manual' ? stored.source : 'manual';
  const record: SmartCullingImageMetadata = {
    ...stored,
    source,
    locked: source !== stored.source || (stored.locked ?? source === 'manual'),
  };
  if (record.rating !== image.rating || record.colorLabel !== imageColor(image)) {
    return {
      ...record,
      source: 'manual',
      locked: true,
      rating: image.rating,
      colorLabel: imageColor(image) as SmartCullingImageMetadata['colorLabel'],
      reasonCodes: undefined,
      reasonText: undefined,
    };
  }
  return record;
}

export function needsManualOwnershipReconciliation(image: ImageFile, effectiveRating = image.rating) {
  const value = image.featureData?.smartCullingV2;
  if (!value || typeof value !== 'object') return effectiveRating > 0 || imageColor(image) !== null;
  const record = value as SmartCullingImageMetadata;
  return record.rating !== effectiveRating || record.colorLabel !== imageColor(image);
}
