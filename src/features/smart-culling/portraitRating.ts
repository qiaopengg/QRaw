/** Serialized states and the actual backend decision; the UI does not score photos. */
export interface PortraitRatingEvidence {
  version: string;
  subjectFaceIndices: number[];
  eyes: 'passed' | 'failed' | 'unableToDetermine';
  expression: 'severeFailure' | 'notRecommended' | 'natural' | 'excellent' | 'outstanding' | 'unableToDetermine';
  opticalAesthetic: {
    optical: 'passed' | 'notPassed' | 'unableToDetermine';
    aestheticComposition: 'passed' | 'notPassed' | 'unableToDetermine';
  };
  status: 'pending' | 'subjectUnclear' | 'scored';
  breakdown: {
    eyeIncrement: number;
    expressionIncrement: number;
    opticalAestheticIncrement: number;
    rawRating: number;
    finalRating: number;
  } | null;
}
