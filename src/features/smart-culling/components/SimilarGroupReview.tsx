import { ArrowLeft, CheckCheck, Layers3, RotateCcw, ShieldAlert, XCircle } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useSmartCullingText } from '../i18n';
import type { ReviewResult } from '../types';
import { ReviewResultCard } from './ReviewResultCard';

function GroupCard({
  result,
  focusedResultId,
  onSelect,
  onToggle,
  onSetComparison,
  readOnly,
}: {
  result: ReviewResult;
  focusedResultId: string | null;
  onSelect: (result: ReviewResult) => void;
  onToggle: (result: ReviewResult) => void;
  onSetComparison: (slot: 'a' | 'b', result: ReviewResult) => void;
  readOnly: boolean;
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
        onToggle={() => onToggle(result)}
        readOnly={readOnly}
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
  onToggle,
  onRestoreInitial,
  onSetAll,
  onSetComparison,
  onRequestThumbnails,
  readOnly,
}: {
  results: ReviewResult[];
  focusedResultId: string | null;
  onBack: () => void;
  onSelect: (result: ReviewResult) => void;
  onToggle: (result: ReviewResult) => void;
  onRestoreInitial: () => void;
  onSetAll: (adopted: boolean) => void;
  onSetComparison: (slot: 'a' | 'b', result: ReviewResult) => void;
  onRequestThumbnails?: (paths: string[]) => void;
  readOnly: boolean;
}) {
  const tx = useSmartCullingText();
  const [expanded, setExpanded] = useState(false);
  const sorted = useMemo(() => [...results].sort((left, right) => left.groupRank - right.groupRank), [results]);
  const selected = sorted.filter((result) => result.adopted);
  const candidates = sorted.filter((result) => !result.adopted);
  const reviewOnly = sorted[0]?.groupKind === 'reviewOnly';

  useEffect(() => {
    onRequestThumbnails?.(sorted.map((result) => result.path));
  }, [onRequestThumbnails, sorted]);

  const cards = (items: ReviewResult[]) =>
    items.map((result) => (
      <GroupCard
        key={result.resultId}
        result={result}
        focusedResultId={focusedResultId}
        onSelect={onSelect}
        onToggle={onToggle}
        onSetComparison={onSetComparison}
        readOnly={readOnly}
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
            {sorted.length} {tx('photoUnit')} · {selected.length} {tx('adopted')}
          </span>
        </div>
        <nav>
          <button disabled={readOnly} onClick={onRestoreInitial}>
            <RotateCcw size={14} />
            {tx('restoreAiSelection')}
          </button>
          <button disabled={readOnly} onClick={() => onSetAll(true)}>
            <CheckCheck size={14} />
            {tx('selectAll')}
          </button>
          <button disabled={readOnly} onClick={() => onSetAll(false)}>
            <XCircle size={14} />
            {tx('clearAll')}
          </button>
        </nav>
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
      {!expanded && !reviewOnly ? (
        <div className="sc-group-collapsed">
          <section>
            <h3>
              {tx('selectedPool')} · {selected.length}
            </h3>
            <div className="sc-group-card-grid">
              {cards(selected)}
              {candidates.length > 0 ? (
                <button className="sc-candidate-stack" onClick={() => setExpanded(true)}>
                  <i />
                  <i />
                  <Layers3 size={28} />
                  <strong>
                    {tx('remainingPhotos')} {candidates.length}
                  </strong>
                  <span>{tx('expandCandidatePool')}</span>
                </button>
              ) : null}
            </div>
          </section>
        </div>
      ) : (
        <div className="sc-group-expanded custom-scrollbar">
          <section>
            <header>
              <h3>
                {tx('selectedPool')} · {selected.length}
              </h3>
              {!reviewOnly ? <button onClick={() => setExpanded(false)}>{tx('collapseCandidates')}</button> : null}
            </header>
            <div className="sc-group-card-grid">{cards(selected)}</div>
          </section>
          {!reviewOnly ? (
            <section>
              <h3>
                {tx('candidatePool')} · {candidates.length}
              </h3>
              <div className="sc-group-card-grid">{cards(candidates)}</div>
            </section>
          ) : null}
        </div>
      )}
    </section>
  );
}
