import { useSmartCullingText, type SmartCullingTextKey } from '../i18n';
import type { OpticalMeasurements, PortraitQualityObservation, QualityUnavailableReason } from '../qualityEvidence';
import '../styles/portrait-quality.css';

const unavailableText: Record<QualityUnavailableReason, SmartCullingTextKey> = {
  invalidFaceBox: 'qualityInvalidFaceBox',
  partialFace: 'qualityPartialFace',
  faceTooSmall: 'qualityFaceTooSmall',
  unreliableDetection: 'qualityUnreliableDetection',
  landmarksUnavailable: 'qualityLandmarksUnavailable',
  strongProfile: 'qualityStrongProfile',
  modelUnavailable: 'qualityModelUnavailable',
  inferenceFailed: 'qualityInferenceFailed',
};

function measured(value: number | null | undefined, digits = 2) {
  return value != null && Number.isFinite(value) ? value.toFixed(digits) : '—';
}

function OpticalStats({ evidence }: { evidence: OpticalMeasurements }) {
  const tx = useSmartCullingText();
  return (
    <dl>
      <div>
        <dt>{tx('qualityHighlights')}</dt>
        <dd>{measured(evidence.highlightClipRatio * 100)}%</dd>
      </div>
      <div>
        <dt>{tx('qualityShadows')}</dt>
        <dd>{measured(evidence.shadowClipRatio * 100)}%</dd>
      </div>
      <div>
        <dt>{tx('qualityMeanRgb')}</dt>
        <dd>{evidence.meanRgb.map((value) => measured(value, 0)).join(' / ')}</dd>
      </div>
    </dl>
  );
}

export function PortraitQualityEvidence({ evidence }: { evidence: PortraitQualityObservation }) {
  const tx = useSmartCullingText();
  return (
    <section className="sc-portrait-quality">
      <h3>{tx('qualityObservationTitle')}</h3>
      <p className="sc-review-evidence-hint">{tx('qualityObservationHint')}</p>
      <details>
        <summary>{tx('qualityFrameOptical')}</summary>
        {evidence.optical ? <OpticalStats evidence={evidence.optical} /> : <p>{tx('qualityNoMeasurements')}</p>}
      </details>
      {evidence.faces.length === 0 ? <p>{tx('qualityNoFaces')}</p> : null}
      {evidence.faces.map((face) => (
        <details key={face.faceIndex}>
          <summary>
            {tx('person')} {face.faceIndex + 1} ·{' '}
            {evidence.subjectFaceIndices.includes(face.faceIndex) ? tx('qualitySubject') : tx('qualityOtherFace')}
          </summary>
          <p>
            {face.qualityUnavailableReason
              ? tx(unavailableText[face.qualityUnavailableReason])
              : tx('qualityModelObserved')}
          </p>
          <dl>
            <div>
              <dt>{tx('qualityFacePixels')}</dt>
              <dd>{face.cropDimensions?.join(' × ') ?? '—'}</dd>
            </div>
            <div>
              <dt>{tx('qualityModelScore')}</dt>
              <dd>{measured(face.qualityScore, 4)}</dd>
            </div>
            <div>
              <dt>{tx('qualityNativeDetail')}</dt>
              <dd>{measured(face.detail?.nativeLaplacian)}</dd>
            </div>
            <div>
              <dt>{tx('qualityScaledDetail')}</dt>
              <dd>{measured(face.detail?.scaledLaplacian)}</dd>
            </div>
          </dl>
          {face.optical ? <OpticalStats evidence={face.optical} /> : null}
          {face.placement ? (
            <>
              <p>{tx('qualityCompositionHint')}</p>
              <dl>
                <div>
                  <dt>{tx('qualityCenterDistance')}</dt>
                  <dd>{measured(face.placement.centerDistance, 3)}</dd>
                </div>
                <div>
                  <dt>{tx('qualityThirdsDistance')}</dt>
                  <dd>{measured(face.placement.thirdsDistance, 3)}</dd>
                </div>
                <div>
                  <dt>{tx('qualityGoldenDistance')}</dt>
                  <dd>{measured(face.placement.goldenDistance, 3)}</dd>
                </div>
              </dl>
            </>
          ) : null}
        </details>
      ))}
      <p className="sc-portrait-quality-version" title={evidence.modelVersion}>
        {evidence.version}
      </p>
    </section>
  );
}
