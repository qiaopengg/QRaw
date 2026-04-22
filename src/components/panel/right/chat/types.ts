import { Adjustments } from '../../../../utils/adjustments';

export const DEFAULT_STYLE_TRANSFER_SERVICE_URL = 'http://127.0.0.1:7860';

export const STYLE_TRANSFER_PRESET_OPTIONS = [
  { label: 'Realistic', value: 'realistic' },
  { label: 'Artistic', value: 'artistic' },
  { label: 'Creative', value: 'creative' },
] as const;

export const STYLE_TRANSFER_EXPORT_FORMAT_OPTIONS = [
  { label: 'TIFF', value: 'tiff' },
  { label: 'PNG', value: 'png' },
  { label: 'JPG', value: 'jpg' },
] as const;

export type StyleTransferModeSetting = 'analysis' | 'generativePreview';
export type StyleTransferRequestMode = StyleTransferModeSetting | 'generativeExport';
export type StyleTransferPreset = (typeof STYLE_TRANSFER_PRESET_OPTIONS)[number]['value'];
export type StyleTransferExportFormat = (typeof STYLE_TRANSFER_EXPORT_FORMAT_OPTIONS)[number]['value'];

export interface AdjustmentSuggestion {
  key: string;
  value: unknown;
  complex_value?: unknown;
  label: string;
  min: number;
  max: number;
  reason: string;
}

export type AdjustmentValue = Adjustments[keyof Adjustments] | unknown;
export type AppliedValueMap = Record<string, AdjustmentValue>;
export type HslPatch = Partial<Record<string, Partial<Adjustments['hsl'][keyof Adjustments['hsl']]>>>;

export interface StyleTransferExecutionMeta {
  requestedMode: string;
  resolvedMode: string;
  engine: string;
  preset: string;
  refineEnabled: boolean;
  usedFallback: boolean;
  stage: string;
  expectedWaitRange: string;
  outputFormat?: string | null;
}

export interface StyleTransferProgressState {
  percentage: number;
  description: string;
  rawText: string;
}

export type StyleTransferPreviewWorkflowState =
  | 'pending'
  | 'preview'
  | 'source'
  | 'compare'
  | 'applied'
  | 'discarded'
  | 'exported';

export type StyleTransferWorkspaceOpenMode =
  | 'default'
  | 'styleTransferCompare'
  | 'styleTransferPreview'
  | 'styleTransferSource'
  | 'styleTransferApply'
  | 'styleTransferDiscard';

export interface ChatOpenImageOptions {
  activatePanel?: 'export';
  preserveAdjustments?: boolean;
  styleTransferSession?: {
    mode: StyleTransferWorkspaceOpenMode;
    sourcePath: string;
    previewPath: string;
    compareBasePath?: string | null;
    compareTargetPath?: string | null;
  };
}

export interface StyleTransferServiceStatus {
  serviceUrl: string;
  reachable: boolean;
  ready: boolean;
  status: string;
  version?: string | null;
  pipeline?: string | null;
  capabilities?: string[];
  detail?: string | null;
}

export interface StyleErrorBreakdown {
  tonal: number;
  color: number;
  skin: number;
  highlight_penalty: number;
  total: number;
}

export interface StyleProximityScore {
  tonal: number;
  color: number;
  skin: number;
  highlight: number;
  overall: number;
}

export interface StyleDebugAction {
  key: string;
  label: string;
  recommended_delta: number;
  priority: number;
  reason: string;
}

export interface StyleConstraintAction {
  key: string;
  label: string;
  delta: number;
}

export interface StyleConstraintBlockItem {
  category: string;
  label: string;
  reason: string;
  hit_count: number;
  severity: number;
  actions: StyleConstraintAction[];
}

export interface StyleSceneProfileDebug {
  reference_tonal_style: string;
  current_tonal_style: string;
  tonal_gain: number;
  highlight_gain: number;
  shadow_gain: number;
  chroma_limit: number;
  chroma_guard_floor: number;
  color_residual_gain: number;
}

export interface ConstraintBand {
  hard_min: number;
  hard_max: number;
  soft_min: number;
  soft_max: number;
}

export interface ConstraintWindow {
  source: string;
  highlight_risk: number;
  shadow_risk: number;
  saturation_risk: number;
  bands: Record<string, ConstraintBand>;
}

export interface ConstraintClampRecord {
  key: string;
  label: string;
  original: number;
  clamped: number;
  reason: string;
}

export interface ConstraintDebugInfo {
  window: ConstraintWindow;
  clamp_count: number;
  clamps: ConstraintClampRecord[];
}

export interface StyleDebugInfo {
  before: StyleErrorBreakdown;
  after: StyleErrorBreakdown;
  proximity_before: StyleProximityScore;
  proximity_after: StyleProximityScore;
  improvement_ratio: number;
  dominant_error: string;
  auto_refine_rounds: number;
  suggested_actions: StyleDebugAction[];
  blocked_reasons: string[];
  blocked_items?: StyleConstraintBlockItem[];
  scene_profile?: StyleSceneProfileDebug;
  constraint_debug?: ConstraintDebugInfo;
}

export interface ChatAdjustResponse {
  understanding: string;
  adjustments: AdjustmentSuggestion[];
  style_debug?: StyleDebugInfo;
  constraint_debug?: ConstraintDebugInfo;
  executionMeta?: StyleTransferExecutionMeta;
  serviceStatus?: StyleTransferServiceStatus;
  outputImagePath?: string;
  previewImagePath?: string;
  pureGenerationImagePath?: string;
  postProcessedImagePath?: string;
}

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  thinkingContent?: string;
  adjustments?: AdjustmentSuggestion[];
  appliedValues?: AppliedValueMap;
  styleDebug?: StyleDebugInfo;
  constraintDebug?: ConstraintDebugInfo;
  executionMeta?: StyleTransferExecutionMeta;
  serviceStatus?: StyleTransferServiceStatus;
  outputImagePath?: string;
  previewImagePath?: string;
  pureGenerationImagePath?: string;
  postProcessedImagePath?: string;
  referencePath?: string;
  sourceImagePath?: string;
  requestedMode?: StyleTransferRequestMode;
  styleTransferProgress?: StyleTransferProgressState;
  previewWorkflowState?: StyleTransferPreviewWorkflowState;
  qualityGuardPassed?: boolean;
}

export interface StreamChunkPayload {
  chunk_type: 'thinking' | 'content' | 'done' | 'error';
  text: string;
  result?: ChatAdjustResponse | null;
}

export interface LlmChatMessage {
  role: string;
  content: string;
}

export type OllamaStatus = 'checking' | 'online' | 'offline';
