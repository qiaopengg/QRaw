import { useSmartCullingText } from '../i18n';
import { expressionLabel } from '../portraitSignalText';
import { reviewResultNeedsAttention } from '../reviewPolicy';
import type { ReviewResult } from '../types';
import { PortraitFaceSignals } from './PortraitFaceSignals';
import '../styles/portrait-rating.css';

function increment(value: number) {
  return value > 0 ? `+${value}` : String(value);
}

export function PortraitRatingEvidence({ result }: { result: ReviewResult }) {
  const tx = useSmartCullingText();
  if (result.mode !== 'portrait') return null;
  const evidence = result.portraitRating;
  if (!evidence)
    return (
      <section>
        <h3>{tx('portraitRatingTitle')}</h3>
        <p>{tx('portraitRatingAbsent')}</p>
      </section>
    );
  const manual = result.source === 'manual';
  const pending = reviewResultNeedsAttention(result) || evidence.status === 'pending';
  const breakdown = !manual && !pending && evidence.status === 'scored' ? evidence.breakdown : null;
  const validationLabel = (state: string) =>
    tx(
      state === 'passed'
        ? 'portraitCheckPassed'
        : state === 'notPassed'
          ? 'portraitCheckFailed'
          : 'portraitCheckUnvalidated',
    );
  return (
    <section className="sc-portrait-rating">
      <h3>{tx('portraitRatingTitle')}</h3>
      <p className="sc-review-evidence-hint">{tx('portraitRatingCalibration')}</p>
      {manual ? (
        <p>{tx('portraitRatingManual')}</p>
      ) : pending ? (
        <p>{tx('portraitRatingPending')}</p>
      ) : evidence.status === 'subjectUnclear' ? (
        <p>{tx('portraitRatingGateStopped')}</p>
      ) : null}
      <p>
        {evidence.subjectFaceIndices.length > 0
          ? `${tx('qualitySubject')}：${evidence.subjectFaceIndices.map((index) => `${tx('person')} ${index + 1}`).join(' / ')}`
          : tx('portraitSubjectUnresolved')}
      </p>
      <dl>
        <div>
          <dt>{tx('portraitEyes')}</dt>
          <dd>
            {tx(
              evidence.eyes === 'passed'
                ? 'portraitEyesPassed'
                : evidence.eyes === 'failed'
                  ? 'portraitEyesFailed'
                  : 'portraitSignalUnknown',
            )}
            {breakdown ? <strong>{increment(breakdown.eyeIncrement)}</strong> : null}
          </dd>
        </div>
        <div>
          <dt>{tx('portraitExpression')}</dt>
          <dd>
            {tx(expressionLabel(evidence.expression))}
            {breakdown ? <strong>{increment(breakdown.expressionIncrement)}</strong> : null}
          </dd>
        </div>
        <div>
          <dt>{tx('portraitOptical')}</dt>
          <dd>{validationLabel(evidence.opticalAesthetic.optical)}</dd>
        </div>
        <div>
          <dt>{tx('portraitComposition')}</dt>
          <dd>{validationLabel(evidence.opticalAesthetic.aestheticComposition)}</dd>
        </div>
        {breakdown ? (
          <>
            <div>
              <dt>{tx('portraitJointIncrement')}</dt>
              <dd>{increment(breakdown.opticalAestheticIncrement)}</dd>
            </div>
            <div className="sc-portrait-rating-total">
              <dt>{tx('portraitRatingTotal')}</dt>
              <dd>
                {breakdown.rawRating} → {breakdown.finalRating}/5
              </dd>
            </div>
          </>
        ) : null}
      </dl>
      <p className="sc-review-evidence-hint">{tx('portraitJointHint')}</p>
      {result.faces.map((face, index) => (
        <details key={index}>
          <summary>
            {tx('person')} {index + 1} ·{' '}
            {tx(evidence.subjectFaceIndices.includes(index) ? 'qualitySubject' : 'qualityOtherFace')}
          </summary>
          <PortraitFaceSignals face={face} />
        </details>
      ))}
    </section>
  );
}
