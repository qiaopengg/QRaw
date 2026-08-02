import { ArrowLeftRight, PanelRightOpen } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useProcessStore } from '../../../store/useProcessStore';
import { useSmartCullingReasonText, useSmartCullingText } from '../i18n';
import type { ReviewResult } from '../types';
import { fileName } from './LifecycleChrome';
import { type EvidenceViewState, PhotoEvidenceViewport } from './PhotoEvidenceViewport';
import { LabelControls, Stars } from './ReviewControls';
import { ReviewFilmstrip } from './ReviewFilmstrip';

const FIT_VIEW: EvidenceViewState = { zoom: 1, pan: { x: 0, y: 0 } };

interface ReviewCompareWorkspaceProps {
  results: ReviewResult[];
  focusedResultId: string;
  onFocus: (resultId: string) => void;
  onToggle: (result: ReviewResult) => void;
  onRating: (result: ReviewResult, rating: number) => void;
  onLabel: (result: ReviewResult, colorLabel: ReviewResult['colorLabel']) => void;
  onOpenInspector: () => void;
  readOnly: boolean;
}

function DecisionControls({
  result,
  onToggle,
  onRating,
  onLabel,
  readOnly,
}: {
  result: ReviewResult;
  onToggle: (result: ReviewResult) => void;
  onRating: (result: ReviewResult, rating: number) => void;
  onLabel: (result: ReviewResult, colorLabel: ReviewResult['colorLabel']) => void;
  readOnly: boolean;
}) {
  const tx = useSmartCullingText();
  return (
    <div className="sc-review-decision-controls">
      <Stars value={result.rating} onChange={readOnly ? undefined : (rating) => onRating(result, rating)} compact />
      <LabelControls
        value={result.colorLabel}
        disabled={readOnly}
        onChange={(colorLabel) => onLabel(result, colorLabel)}
      />
      <button
        className={`sc-review-keep ${result.adopted ? 'is-checked' : ''}`}
        disabled={readOnly}
        onClick={() => onToggle(result)}
        aria-pressed={result.adopted}
      >
        {result.adopted ? tx('adopted') : tx('notAdopted')}
      </button>
    </div>
  );
}

export function ReviewCompareWorkspace({
  results,
  focusedResultId,
  onFocus,
  onToggle,
  onRating,
  onLabel,
  onOpenInspector,
  readOnly,
}: ReviewCompareWorkspaceProps) {
  const tx = useSmartCullingText();
  const reason = useSmartCullingReasonText();
  const thumbnails = useProcessStore((state) => state.thumbnails);
  const [syncView, setSyncView] = useState(true);
  const [sharedView, setSharedView] = useState<EvidenceViewState>(FIT_VIEW);
  const [secondaryResultId, setSecondaryResultId] = useState<string | null>(null);
  const [activePane, setActivePane] = useState<'a' | 'b'>('a');
  const focused = results.find((result) => result.resultId === focusedResultId) ?? results[0];
  const secondary = useMemo(
    () =>
      results.find((result) => result.resultId === secondaryResultId && result.resultId !== focused?.resultId) ??
      results.find((result) => result.resultId !== focused?.resultId),
    [focused?.resultId, results, secondaryResultId],
  );

  useEffect(() => {
    setSharedView(FIT_VIEW);
    setActivePane('a');
  }, [focused?.groupId]);

  useEffect(() => {
    const hasValidSecondary = results.some(
      (result) => result.resultId === secondaryResultId && result.resultId !== focused?.resultId,
    );
    if (!hasValidSecondary) {
      setSecondaryResultId(secondary?.resultId ?? null);
    }
  }, [focused?.resultId, results, secondary?.resultId, secondaryResultId]);

  if (!focused || !secondary) return null;

  const selectFilmstripResult = (resultId: string) => {
    if (resultId === focused.resultId) {
      setActivePane('a');
      return;
    }
    if (resultId === secondary.resultId) {
      setActivePane('b');
      return;
    }
    if (activePane === 'b') {
      setSecondaryResultId(resultId);
      return;
    }
    setSecondaryResultId(focused.resultId);
    onFocus(resultId);
  };

  const swapComparison = () => {
    const previousFocusedId = focused.resultId;
    onFocus(secondary.resultId);
    setSecondaryResultId(previousFocusedId);
    setActivePane('a');
  };

  const panes: Array<{ slot: 'a' | 'b'; result: ReviewResult }> = [
    { slot: 'a', result: focused },
    { slot: 'b', result: secondary },
  ];

  return (
    <section className="sc-decision-workspace is-comparison">
      <header className="sc-decision-header">
        <div>
          <strong>
            {tx('compareGroup')} · {results.length} {tx('photoUnit')}
          </strong>
          <span>
            {tx('compareReviewHint')} · {tx('recommended')} {focused.recommendedCount}
          </span>
        </div>
        <div className="sc-compare-actions">
          <div className="sc-ab-mobile-switch" role="group" aria-label={tx('comparisonPane')}>
            <button
              className={activePane === 'a' ? 'is-active' : ''}
              onClick={() => setActivePane('a')}
              aria-pressed={activePane === 'a'}
            >
              A
            </button>
            <button
              className={activePane === 'b' ? 'is-active' : ''}
              onClick={() => setActivePane('b')}
              aria-pressed={activePane === 'b'}
            >
              B
            </button>
          </div>
          <button onClick={swapComparison} aria-label={tx('swapComparison')}>
            <ArrowLeftRight size={14} />
            <span>{tx('swapComparison')}</span>
          </button>
          <button
            className={syncView ? 'is-active' : ''}
            onClick={() => {
              setSyncView((current) => !current);
              setSharedView(FIT_VIEW);
            }}
            aria-pressed={syncView}
          >
            {syncView ? tx('syncViewOn') : tx('syncViewOff')}
          </button>
          <button onClick={onOpenInspector}>
            <PanelRightOpen size={14} />
            <span>{tx('viewEvidence')}</span>
          </button>
        </div>
      </header>
      <div className="sc-ab-grid">
        {panes.map(({ slot, result }) => (
          <article
            key={`${slot}-${result.resultId}`}
            className={`sc-ab-pane is-${slot} ${activePane === slot ? 'is-active-mobile' : ''}`}
            onPointerDown={() => setActivePane(slot)}
          >
            <div className="sc-ab-photo">
              <span className="sc-ab-slot">{slot.toUpperCase()}</span>
              <PhotoEvidenceViewport
                path={result.path}
                fallbackUrl={thumbnails[result.path]}
                alt={fileName(result.path)}
                faces={result.faces}
                compact
                viewState={syncView ? sharedView : undefined}
                onViewStateChange={syncView ? setSharedView : undefined}
              />
            </div>
            <footer>
              <div className="sc-ab-result-copy">
                <strong title={fileName(result.path)}>{fileName(result.path)}</strong>
                <span>{result.source === 'ai' ? reason(result.reasonCodes) : tx('manualReason')}</span>
              </div>
              <DecisionControls
                result={result}
                onToggle={onToggle}
                onRating={onRating}
                onLabel={onLabel}
                readOnly={readOnly}
              />
            </footer>
          </article>
        ))}
      </div>
      <ReviewFilmstrip
        results={results}
        focusedResultId={focused.resultId}
        secondaryResultId={secondary.resultId}
        onSelect={selectFilmstripResult}
        onToggle={onToggle}
        onRating={onRating}
        readOnly={readOnly}
      />
    </section>
  );
}
