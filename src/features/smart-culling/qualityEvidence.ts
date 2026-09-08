export interface OpticalMeasurements {
  sampleCount: number;
  sampleStride: number;
  shadowClipRatio: number;
  highlightClipRatio: number;
  channelShadowClipRatio: [number, number, number];
  channelHighlightClipRatio: [number, number, number];
  meanRgb: [number, number, number];
  lumaP05: number;
  lumaP50: number;
  lumaP95: number;
  lumaStdDev: number;
}

export interface FaceDetailMeasurements {
  nativeDimensions: [number, number];
  scaledDimensions: [number, number];
  nativeLaplacian: number;
  scaledLaplacian: number;
  scaledSobelEnergy: number;
  blurRetentionRatio: number | null;
}

export type QualityUnavailableReason =
  | 'invalidFaceBox'
  | 'partialFace'
  | 'faceTooSmall'
  | 'unreliableDetection'
  | 'landmarksUnavailable'
  | 'strongProfile'
  | 'modelUnavailable'
  | 'inferenceFailed';

export interface FaceQualityObservation {
  faceIndex: number;
  cropDimensions: [number, number] | null;
  detail: FaceDetailMeasurements | null;
  optical: OpticalMeasurements | null;
  placement: {
    center: [number, number];
    frameAreaRatio: number;
    topMarginRatio: number;
    centerDistance: number;
    thirdsDistance: number;
    goldenDistance: number;
  } | null;
  yawRatio: number | null;
  rollDegrees: number | null;
  qualityScore: number | null;
  qualityUnavailableReason: QualityUnavailableReason | null;
}

export interface PortraitQualityObservation {
  version: string;
  modelVersion: string;
  inputSource: 'renderedRgb8' | 'decodedRgb8';
  inputDimensions: [number, number];
  optical: OpticalMeasurements | null;
  faces: FaceQualityObservation[];
  subjectFaceIndices: number[];
}
