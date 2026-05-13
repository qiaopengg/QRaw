export type SmartCullingRange = 'current_folder' | 'selected' | 'current_filter';
export type SmartCullingMode =
  | 'portrait'
  | 'wedding_event'
  | 'family_children'
  | 'landscape'
  | 'street_documentary'
  | 'sports_wildlife'
  | 'product_still'
  | 'architecture'
  | 'general';
export type SmartCullingPreset = 'strict' | 'balanced' | 'loose';
export type SmartCullingAestheticPreference = 'general' | 'dark_tone' | 'film' | 'shallow_depth' | 'candid_emotion';
export type SmartCullingFaceCheck =
  | 'closed_eyes'
  | 'blurred_face'
  | 'abnormal_expression'
  | 'smile'
  | 'best_group_expression'
  | 'looking_camera';
export type SmartCullingColorLabel = 'red' | 'yellow' | 'green' | 'blue' | 'purple' | 'none';
export type SmartCullingStatus = 'selected' | 'review' | 'reject_suggestion' | 'skipped' | 'failed';

export interface SmartCullingModelsStatus {
  modelsDir: string;
  manifestFound: boolean;
  canRunFull: boolean;
  canRunBasic: boolean;
  degradedReason?: string | null;
  missingRequired: string[];
  missingOptional: string[];
}

export interface SmartCullingStartParams {
  paths: string[];
  mode: SmartCullingMode;
  preset: SmartCullingPreset;
  aestheticPreference: SmartCullingAestheticPreference;
  faceChecks: SmartCullingFaceCheck[];
  includeEdited: boolean;
  previewOnly: boolean;
  keepPerGroup: number;
  faceAnalysisEnabled: boolean;
  allowDegraded: boolean;
}

export interface SmartCullingPresetConfig {
  mode: SmartCullingMode;
  preset: SmartCullingPreset;
  aestheticPreference: SmartCullingAestheticPreference;
  includeEdited: boolean;
  previewOnly: boolean;
  keepPerGroup: number;
  faceAnalysisEnabled: boolean;
  faceChecks: SmartCullingFaceCheck[];
}

export interface SmartCullingUserPreset {
  id: string;
  name: string;
  createdAt: string;
  updatedAt: string;
  config: SmartCullingPresetConfig;
}

export interface SmartCullingSavePresetParams {
  id?: string | null;
  name: string;
  config: SmartCullingPresetConfig;
}

export interface SmartCullingProgress {
  taskId: string;
  current: number;
  total: number;
  stage: string;
}

export interface SmartCullingReviewItem {
  path: string;
  fileName: string;
  rating: number;
  status: SmartCullingStatus;
  colorLabel?: SmartCullingColorLabel | null;
  score: number;
  confidence: number;
  degraded: boolean;
  reasonCodes: string[];
  reasonText: string;
  groupId?: string | null;
  groupRank?: number | null;
  groupSize?: number | null;
  skipReason?: string | null;
}

export interface SmartCullingSummary {
  analyzed: number;
  skipped: number;
  selected: number;
  review: number;
  rejectSuggestion: number;
  failed: number;
}

export interface SmartCullingTaskResult {
  taskId: string;
  status: 'running' | 'review_ready' | 'applied' | 'cancelled' | 'failed' | 'discarded' | 'revoked';
  previewOnly: boolean;
  degraded: boolean;
  createdAt: string;
  appliedAt?: string | null;
  revokedAt?: string | null;
  reportPath?: string | null;
  error?: string | null;
  summary: SmartCullingSummary;
  items: SmartCullingReviewItem[];
}

export interface SmartCullingApplyResult {
  taskId: string;
  applied: number;
  skipped: number;
  appliedPaths: string[];
  skippedPaths: string[];
  reportPath?: string | null;
}

export interface SmartCullingHistoryItem {
  taskId: string;
  status: SmartCullingTaskResult['status'];
  createdAt: string;
  summary: SmartCullingSummary;
  degraded: boolean;
  previewOnly: boolean;
  reportPath?: string | null;
}

export interface SmartCullingReportResult {
  taskId: string;
  reportPath: string;
}

export interface SmartCullingUndoResult {
  taskId: string;
  restored: number;
  skipped: number;
}
