import { Check, Images, ShieldAlert, UserRoundSearch } from 'lucide-react';
import { useProcessStore } from '../../../store/useProcessStore';
import { useSmartCullingReasonText, useSmartCullingText } from '../i18n';
import type { ReviewResult } from '../types';
import { fileName } from './LifecycleChrome';
import { Stars } from './ReviewControls';

export function ReviewResultCard({
  result,
  width,
  imageHeight,
  selected,
  onSelect,
  onToggle,
  onOpenGroup,
  readOnly,
}: {
  result: ReviewResult;
  width: number;
  imageHeight: number;
  selected: boolean;
  onSelect: () => void;
  onToggle: () => void;
  onOpenGroup?: () => void;
  readOnly: boolean;
}) {
  const tx = useSmartCullingText();
  const reason = useSmartCullingReasonText();
  const thumbnail = useProcessStore((state) => state.thumbnails[result.path]);
  const keyPersonCandidate = result.keyPersonEvidence.some((evidence) => evidence.faceIndex !== null);

  return (
    <article
      className={`sc-gallery-card ${selected ? 'is-selected' : ''} ${result.adopted ? 'is-adopted' : 'is-not-adopted'}`}
      style={{ width }}
    >
      <button className="sc-gallery-preview" style={{ height: imageHeight }} onClick={onSelect}>
        {thumbnail ? (
          <img src={thumbnail} alt={fileName(result.path)} />
        ) : (
          <span className="sc-gallery-placeholder">{fileName(result.path)}</span>
        )}
        <i className="sc-gallery-source">{result.source === 'ai' ? 'AI' : tx('manual')}</i>
        <span className={`sc-gallery-adoption ${result.adopted ? 'is-adopted' : ''}`}>
          {result.adopted ? <Check size={12} /> : null}
          {result.adopted ? tx('adopted') : tx('notAdopted')}
        </span>
      </button>
      <div className="sc-gallery-card-body">
        <div className="sc-gallery-card-title">
          <strong title={fileName(result.path)}>{fileName(result.path)}</strong>
          {result.groupKind === 'reviewOnly' ? (
            <span title={tx('reviewOnlyHint')}>
              <ShieldAlert size={12} />
              {tx('reviewOnly')}
            </span>
          ) : result.groupKind === 'similar' ? (
            <button onClick={onOpenGroup}>
              <Images size={12} />
              {tx('similarGroup')} {result.groupIndex}
            </button>
          ) : (
            <span>{tx('singlePhoto')}</span>
          )}
        </div>
        <div className="sc-gallery-card-status">
          <Stars value={result.rating} compact />
          <i className={`sc-color-dot is-${result.colorLabel ?? 'none'}`} />
          {keyPersonCandidate ? (
            <span title={tx('identityPending')}>
              <UserRoundSearch size={12} />
              {tx('keyPersonCandidate')}
            </span>
          ) : null}
        </div>
        <p title={result.source === 'ai' ? reason(result.reasonCodes) : tx('manualReason')}>
          {result.source === 'ai' ? reason(result.reasonCodes) : tx('manualReason')}
        </p>
        <button
          className={`sc-gallery-toggle ${result.adopted ? 'is-adopted' : ''}`}
          disabled={readOnly}
          onClick={onToggle}
        >
          {result.adopted ? tx('moveToCandidates') : tx('moveToSelected')}
        </button>
      </div>
    </article>
  );
}
