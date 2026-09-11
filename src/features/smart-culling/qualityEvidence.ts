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
  clarity?: ClarityObservation | null;
}

export type ClarityInputUnavailable =
  | 'notRequested'
  | 'rawSourceUnvalidated'
  | 'contentEdited'
  | 'geometryUnvalidated'
  | 'invalidAdjustments'
  | 'invalidCrop'
  | 'decodeFailed'
  | 'renderDimensionsMismatch';

export interface ClarityObservation {
  version: string;
  inputSource: 'decodedRgb8LosslessGeometry';
  inputDimensions: [number, number] | null;
  sourceSha256: string | null;
  adjustmentsSha256: string | null;
  unavailableReason: ClarityInputUnavailable | null;
  faces: Array<{
    faceIndex: number;
    bbox: [number, number, number, number];
    detail: FaceDetailMeasurements | null;
    robustDetail: {
      version: string;
      noiseSigma: number;
      medianRadius: number;
      denoisedDetail: FaceDetailMeasurements;
    } | null;
    unavailableReason: QualityUnavailableReason | null;
  }>;
}
