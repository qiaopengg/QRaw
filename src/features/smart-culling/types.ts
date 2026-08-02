import type { SmartCullingMode } from './constants';

export type SmartCullingState =
  | 'idle'
  | 'configuring'
  | 'indexing'
  | 'rendering'
  | 'analyzing'
  | 'organizing'
  | 'cancelling'
  | 'readyForReview'
  | 'confirming'
  | 'completed'
  | 'unsupported'
  | 'failed';

export interface KeyPersonSelection {
  samplePath: string;
  bbox: [number, number, number, number];
  priority: number;
}

export interface DetectedFace {
  bbox: [number, number, number, number];
  score: number;
  thumbnailDataUrl: string | null;
  landmarks?: [[number, number], [number, number], [number, number], [number, number], [number, number]] | null;
  leftEye?: EyeEvidence | null;
  rightEye?: EyeEvidence | null;
  expressionState?: string | null;
  expressionConfidence?: number | null;
  expressionReason?: string | null;
  sharpnessMetric?: number | null;
  sharpnessConfidence?: number | null;
  exposureMetric?: number | null;
  exposureConfidence?: number | null;
}

export interface EyeEvidence {
  openProbability: number | null;
  state: 'open' | 'closed' | 'unknown';
  confidence: number;
  effectivePixels: number;
  sharpnessMetric: number | null;
}

export interface KeyPersonEvidence {
  priority: number;
  faceIndex: number | null;
  similarity: number | null;
  status: 'candidate' | 'ambiguous' | 'not_found' | 'reference_unavailable';
  autoScoreEligible: boolean;
  performanceRank: number | null;
}

export interface DevicePreflight {
  checked: boolean;
  supported: boolean;
  platform: string;
  provider: string;
  modelVersion: string;
  reason: string | null;
}

export interface InventorySummary {
  totalAssets: number;
  eligibleAssets: number;
  protectedAssets: number;
  skippedAssets: number;
  failedAssets: number;
  folderCount: number;
}

export interface TaskProgress {
  completed: number;
  total: number;
  percent: number;
  stage: string;
  etaSeconds: number | null;
  partial: boolean;
}

export interface ReviewResult {
  resultId: string;
  path: string;
  memberPaths: string[];
  folder: string;
  groupId: string;
  groupKind: 'similar' | 'single' | 'reviewOnly';
  groupIndex: number;
  groupRank: number;
  groupSize: number;
  recommendedCount: number;
  rating: number;
  colorLabel: 'green' | 'yellow' | 'red' | null;
  source: 'ai' | 'manual';
  mode: SmartCullingMode;
  reasonCodes: string[];
  confidence: number;
  aiInitiallyAdopted: boolean;
  adopted: boolean;
  protected: boolean;
  width: number;
  height: number;
  faces: DetectedFace[];
  keyPersonEvidence: KeyPersonEvidence[];
}

export interface FailureItem {
  path: string;
  memberPaths: string[];
  stage: string;
  code: string;
  detail: string;
  retryable: boolean;
}

export interface SmartCullingCommandError {
  code: string;
  detail: string;
}

export interface WriteSummary {
  succeeded: number;
  failed: number;
  protected: number;
  skipped: number;
  succeededPaths: string[];
}

export interface SmartCullingSnapshot {
  taskId: string | null;
  state: SmartCullingState;
  rootPath: string | null;
  mode: SmartCullingMode | null;
  device: DevicePreflight;
  inventory: InventorySummary;
  progress: TaskProgress;
  results: ReviewResult[];
  failures: FailureItem[];
  detectedImagePath: string | null;
  detectedFaces: DetectedFace[];
  writeSummary: WriteSummary | null;
}

export interface ReviewChange {
  resultId: string;
  adopted: boolean;
  rating: number;
  colorLabel: ReviewResult['colorLabel'];
  mode: SmartCullingMode;
  metadataEdited: boolean;
  modeChanged: boolean;
}

export type SmartCullingRequest =
  | { action: 'status' }
  | { action: 'inspect'; rootPath: string }
  | { action: 'detectPeople'; path: string }
  | { action: 'start'; rootPath: string; mode: SmartCullingMode; keyPeople: KeyPersonSelection[] }
  | { action: 'cancel' }
  | { action: 'updateReview'; changes: ReviewChange[] }
  | { action: 'confirm' }
  | { action: 'retryFailures' }
  | { action: 'reconcileManual'; paths: string[] }
  | { action: 'abandon' };

export type LifecycleScreen = 'setup' | 'analysis' | 'review' | 'write' | 'unsupported';
