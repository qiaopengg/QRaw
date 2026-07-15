export const SMART_CULLING_REVIEW_VIEW = 'smart-culling-review';

export const SMART_CULLING_INVOKES = {
  Analyze: 'smart_culling_analyze',
  WriteMetadata: 'smart_culling_write_metadata',
} as const;

export const SMART_CULLING_DEFAULT_SETTINGS = {
  similarityThreshold: 28,
  blurThreshold: 100.0,
  groupSimilar: true,
  filterBlurry: true,
  detectFaces: false,
} as const;
