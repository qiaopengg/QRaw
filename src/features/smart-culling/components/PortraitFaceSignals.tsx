import { useSmartCullingText } from '../i18n';
import { expressionLabel, signalConfidence, signalReason } from '../portraitSignalText';
import type { DetectedFace, EyeEvidence } from '../types';

function EyeSignal({ eye }: { eye: EyeEvidence | null | undefined }) {
  const tx = useSmartCullingText();
  const known = eye?.state === 'open' || eye?.state === 'closed';
  return (
    <>
      {tx(
        eye?.state === 'open'
          ? 'portraitEyeOpen'
          : eye?.state === 'closed'
            ? 'portraitEyeClosed'
            : 'portraitSignalUnknown',
      )}
      {known ? (
        <small>
          {tx('portraitConfidence')} {signalConfidence(eye?.confidence)}
        </small>
      ) : (
        <small>{tx(signalReason(eye?.reason))}</small>
      )}
    </>
  );
}

export function PortraitFaceSignals({ face }: { face: DetectedFace }) {
  const tx = useSmartCullingText();
  const expression = expressionLabel(face.expressionState);
  return (
    <dl>
      <div>
        <dt>{tx('portraitLeftEye')}</dt>
        <dd>
          <EyeSignal eye={face.leftEye} />
        </dd>
      </div>
      <div>
        <dt>{tx('portraitRightEye')}</dt>
        <dd>
          <EyeSignal eye={face.rightEye} />
        </dd>
      </div>
      <div>
        <dt>{tx('portraitExpression')}</dt>
        <dd>
          {tx(expression)}
          <small>
            {expression === 'portraitSignalUnknown'
              ? tx(signalReason(face.expressionReason))
              : `${tx('portraitConfidence')} ${signalConfidence(face.expressionConfidence)}`}
          </small>
        </dd>
      </div>
    </dl>
  );
}
