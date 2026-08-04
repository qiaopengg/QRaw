import { ArrowLeft, ShieldAlert } from 'lucide-react';
import { useEffect, useMemo } from 'react';
import { useSmartCullingText } from '../i18n';
import type { ReviewResult } from '../types';
import { ReviewResultCard } from './ReviewResultCard';

function GroupCard({
  result,
  focusedResultId,
  onSelect,
  onSetComparison,
}: {
  result: ReviewResult;
  focusedResultId: string | null;
  onSelect: (result: ReviewResult) => void;
  onSetComparison: (slot: 'a' | 'b', result: ReviewResult) => void;
}) {
  const tx = useSmartCullingText();
  const width = 220;
  const imageHeight = Math.min(260, Math.max(150, width * (result.height / Math.max(result.width, 1))));
  return (
    <div className="sc-group-card-shell">
      <ReviewResultCard
        result={result}
        width={width}
        imageHeight={imageHeight}
        selected={focusedResultId === result.resultId}
        onSelect={() => onSelect(result)}
      />
      {result.groupKind === 'similar' ? (
        <div className="sc-group-ab-actions">
          <button onClick={() => onSetComparison('a', result)}>{tx('setAsA')}</button>
          <button onClick={() => onSetComparison('b', result)}>{tx('setAsB')}</button>
        </div>
      ) : null}
    </div>
  );
}

export function SimilarGroupReview({
  results,
  focusedResultId,
  onBack,
  onSelect,
  onSetComparison,
  onRequestThumbnails,
}: {
  results: ReviewResult[];
  focusedResultId: string | null;
  onBack: () => void;
  onSelect: (result: ReviewResult) => void;
  onSetComparison: (slot: 'a' | 'b', result: ReviewResult) => void;
  onRequestThumbnails?: (paths: string[]) => void;
}) {
  const tx = useSmartCullingText();
  const sorted = useMemo(() => [...results].sort((left, right) => left.groupRank - right.groupRank), [results]);
  const reviewOnly = sorted[0]?.groupKind === 'reviewOnly';

  useEffect(() => {
    onRequestThumbnails?.(sorted.map((result) => result.path));
  }, [onRequestThumbnails, sorted]);

  const cards = sorted.map((result) => (
    <GroupCard
      key={result.resultId}
      result={result}
      focusedResultId={focusedResultId}
      onSelect={onSelect}
      onSetComparison={onSetComparison}
    />
  ));

  return (
    <section className="sc-group-review">
      <header className="sc-group-review-header">
        <button className="sc-group-back" onClick={onBack}>
          <ArrowLeft size={15} />
          {tx('backToAllResults')}
        </button>
        <div>
          <h2>{reviewOnly ? tx('reviewOnly') : `${tx('similarGroup')} ${sorted[0]?.groupIndex ?? ''}`}</h2>
          <span>
            {sorted.length} {tx('photoUnit')} · {tx('rankedByQuality')}
          </span>
        </div>
      </header>
      {reviewOnly ? (
        <div className="sc-review-only-banner">
          <ShieldAlert size={18} />
          <div>
            <strong>{tx('reviewOnlyTitle')}</strong>
            <p>{tx('reviewOnlyHint')}</p>
          </div>
        </div>
      ) : null}
      <div className="sc-group-expanded custom-scrollbar">
        <section>
          <div className="sc-group-card-grid">{cards}</div>
        </section>
      </div>
    </section>
  );
}
