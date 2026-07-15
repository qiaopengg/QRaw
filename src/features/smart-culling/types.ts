export interface SmartCullingSettings {
  similarityThreshold: number;
  blurThreshold: number;
  groupSimilar: boolean;
  filterBlurry: boolean;
  detectFaces: boolean;
}

export interface FaceResult {
  bbox: [number, number, number, number];
  eyeOpenProb: number | null;
  isClosed: boolean;
}

export interface ImageAnalysisResult {
  path: string;
  qualityScore: number;
  sharpnessMetric: number;
  centerFocusMetric: number;
  exposureMetric: number;
  width: number;
  height: number;
  faces: FaceResult[];
}

export interface CullGroup {
  representative: ImageAnalysisResult;
  duplicates: ImageAnalysisResult[];
}

export interface SmartCullingSuggestions {
  similarGroups: CullGroup[];
  blurryImages: ImageAnalysisResult[];
  problemFaces: ImageAnalysisResult[];
  failedPaths: string[];
}

export interface SmartCullingProgress {
  current: number;
  total: number;
  stage: string;
}

export type SmartCullingApplyAction = 'reject' | 'rate_zero' | 'delete';

export interface SmartCullingApplyItem {
  path: string;
  score: number;
  reasonText: string;
  status: string;
}
