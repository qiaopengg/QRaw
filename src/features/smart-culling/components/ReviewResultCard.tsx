import { Images, LockKeyhole, ShieldAlert, UserRoundSearch } from 'lucide-react';
import { useProcessStore } from '../../../store/useProcessStore';
import { useSmartCullingReasonText, useSmartCullingText } from '../i18n';
import type { ReviewResult } from '../types';
import { fileName } from './LifecycleChrome';
import { Stars } from './ReviewControls';
import { SmartCullingImage } from './SmartCullingImage';

export function ReviewResultCard({
  result,
  width,
  imageHeight,
  selected,
  onSelect,
  onOpenGroup,
}: {
  result: ReviewResult;
  width: number;
  imageHeight: number;
  selected: boolean;
  onSelect: () => void;
  onOpenGroup?: () => void;
}) {
  const tx = useSmartCullingText();
  const reason = useSmartCullingReasonText();
  const thumbnail = useProcessStore((state) => state.thumbnails[result.path]);
  const keyPersonNeedsReview = result.keyPersonEvidence.some((evidence) =>
    ['suspected', 'ambiguous', 'unknown'].includes(evidence.status),
  );

  return (
    <article className={`sc-gallery-card ${selected ? 'is-selected' : ''}`} style={{ width }}>
      <button className="sc-gallery-preview" style={{ height: imageHeight }} onClick={onSelect}>
        {thumbnail ? (
          <SmartCullingImage
            primaryUrl={thumbnail}
            alt={fileName(result.path)}
            fallback={<span className="sc-gallery-placeholder">{fileName(result.path)}</span>}
          />
        ) : (
          <span className="sc-gallery-placeholder">{fileName(result.path)}</span>
        )}
        <i className="sc-gallery-source">{result.source === 'ai' ? 'AI' : tx('manual')}</i>
        {result.groupSize > 1 ? (
          <span className="sc-gallery-rank">
            {tx('groupRank')} {result.groupRank}/{result.groupSize}
          </span>
        ) : null}
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
          {result.protected ? (
            <span title={tx('lockedResultHint')}>
              <LockKeyhole size={12} />
              {tx('lockedResult')}
            </span>
          ) : null}
          {keyPersonNeedsReview ? (
            <span title={tx('identityPending')}>
              <UserRoundSearch size={12} />
              {tx('suspectedKeyPerson')}
            </span>
          ) : null}
        </div>
        <p title={result.source === 'ai' ? reason(result.reasonCodes) : tx('manualReason')}>
          {result.source === 'ai' ? reason(result.reasonCodes) : tx('manualReason')}
        </p>
      </div>
    </article>
  );
}
