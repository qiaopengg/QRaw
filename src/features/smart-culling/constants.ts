export const SMART_CULLING_VIEW = 'smart-culling-review';
export const SMART_CULLING_COMMAND = 'smart_culling_command';
export const SMART_CULLING_EVENT = 'smart-culling://event';
export const SMART_CULLING_RUNNING_STATES = ['indexing', 'rendering', 'analyzing', 'organizing', 'cancelling'] as const;

export const SMART_CULLING_MODES = [
  'auto',
  'landscape',
  'portrait',
  'environment',
  'group',
  'documentary',
  'wildlife',
  'architecture',
  'product',
  'astro',
] as const;

export type SmartCullingMode = (typeof SMART_CULLING_MODES)[number];
