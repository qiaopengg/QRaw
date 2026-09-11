import { useSmartCullingText, type SmartCullingTextKey } from '../i18n';
import type { ClarityInputUnavailable, ClarityObservation } from '../qualityEvidence';

const unavailableText: Record<ClarityInputUnavailable, SmartCullingTextKey> = {
  notRequested: 'qualityClarityNotRequested',
  rawSourceUnvalidated: 'qualityClarityRaw',
  contentEdited: 'qualityClarityEdited',
  geometryUnvalidated: 'qualityClarityGeometry',
  invalidAdjustments: 'qualityClarityAdjustments',
  invalidCrop: 'qualityClarityCrop',
  decodeFailed: 'qualityClarityDecode',
  renderDimensionsMismatch: 'qualityClarityMismatch',
};

export function PortraitClarityEvidence({ evidence }: { evidence?: ClarityObservation | null }) {
  const tx = useSmartCullingText();
  const available = evidence?.inputSource === 'decodedRgb8LosslessGeometry' && !evidence.unavailableReason;
  const measured = (value?: number | null) => (value != null && Number.isFinite(value) ? value.toFixed(2) : '—');
  return (
    <details>
      <summary>{tx('qualityClarityTitle')}</summary>
      <p>{tx('qualityClarityHint')}</p>
      <p>
        {evidence?.unavailableReason
          ? tx(unavailableText[evidence.unavailableReason] ?? 'qualityClarityAbsent')
          : tx(available ? 'qualityClarityAvailable' : 'qualityClarityAbsent')}
      </p>
      {available
        ? evidence.faces.map((face) => (
            <div key={face.faceIndex}>
              <strong>
                {tx('person')} {face.faceIndex + 1}
              </strong>
              {face.unavailableReason || !face.detail ? (
                <p>{tx('qualityClarityFaceAbsent')}</p>
              ) : (
                <dl>
                  <div>
                    <dt>{tx('qualityClarityPixels')}</dt>
                    <dd>{face.detail.nativeDimensions.join(' × ')}</dd>
                  </div>
                  <div>
                    <dt>{tx('qualityClarityDetail')}</dt>
                    <dd>{measured(face.detail.nativeLaplacian)}</dd>
                  </div>
                  <div>
                    <dt>{tx('qualityClarityFiltered')}</dt>
                    <dd>{measured(face.robustDetail?.denoisedDetail.nativeLaplacian)}</dd>
                  </div>
                </dl>
              )}
            </div>
          ))
        : null}
    </details>
  );
}
