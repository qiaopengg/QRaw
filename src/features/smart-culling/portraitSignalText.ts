import type { SmartCullingTextKey } from './i18n';

const expressionLabels: Record<string, SmartCullingTextKey> = {
  severe_failure: 'portraitExpressionSevere',
  severeFailure: 'portraitExpressionSevere',
  not_recommended: 'portraitExpressionPoor',
  notRecommended: 'portraitExpressionPoor',
  natural: 'portraitExpressionNatural',
  excellent: 'portraitExpressionExcellent',
  outstanding: 'portraitExpressionOutstanding',
};

export function expressionLabel(state: string | null | undefined): SmartCullingTextKey {
  return expressionLabels[state ?? ''] ?? 'portraitSignalUnknown';
}

export function signalReason(reason: string | null | undefined): SmartCullingTextKey {
  switch (reason) {
    case 'eye_face_presence_unreliable':
      return 'portraitFacePresenceUnreliable';
    case 'eye_pose_unreliable':
      return 'portraitPoseUnreliable';
    case 'eye_resolution_insufficient':
      return 'portraitResolutionInsufficient';
    case 'eye_model_input_unvalidated':
    case 'expression_model_unvalidated':
    case 'expression_quality_model_unavailable':
      return 'portraitSignalModelUnavailable';
    case 'expression_requires_sequence_context':
      return 'portraitSequenceUnavailable';
    case 'expression_quality_model_output_invalid':
    case 'expression_frame_evidence_invalid':
      return 'portraitSignalInvalid';
    default:
      return 'portraitSignalInsufficient';
  }
}

export function signalConfidence(value: number | null | undefined): string {
  return value != null && Number.isFinite(value) && value >= 0 && value <= 1 ? value.toFixed(2) : '—';
}
