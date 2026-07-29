import { Check, ChevronLeft, ChevronRight } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useProcessStore } from '../../../store/useProcessStore';
import { useSmartCullingText } from '../i18n';
import type { ReviewResult } from '../types';
import { fileName } from './LifecycleChrome';
import { Stars } from './ReviewControls';

function FilmstripPreview({ result, fallbackUrl }: { result: ReviewResult; fallbackUrl?: string }) {
  return fallbackUrl ? <img src={fallbackUrl} alt={fileName(result.path)} /> : <span>{fileName(result.path)}</span>;
}

export function ReviewFilmstrip({
  results,
  focusedResultId,
  secondaryResultId,
  onSelect,
  onToggle,
  onRating,
  readOnly,
}: {
  results: ReviewResult[];
  focusedResultId: string;
  secondaryResultId: string | null;
  onSelect: (resultId: string) => void;
  onToggle: (result: ReviewResult) => void;
  onRating: (result: ReviewResult, rating: number) => void;
  readOnly: boolean;
}) {
  const tx = useSmartCullingText();
  const thumbnails = useProcessStore((state) => state.thumbnails);
  const stripRef = useRef<HTMLDivElement>(null);
  const [scrollState, setScrollState] = useState({ left: false, right: false });
  const updateScrollState = useCallback(() => {
    const strip = stripRef.current;
    if (!strip) return;
    setScrollState({
      left: strip.scrollLeft > 1,
      right: strip.scrollLeft + strip.clientWidth < strip.scrollWidth - 1,
    });
  }, []);
  useEffect(() => {
    const strip = stripRef.current;
    if (!strip) return;
    updateScrollState();
    const observer = new ResizeObserver(updateScrollState);
    observer.observe(strip);
    return () => observer.disconnect();
  }, [results.length, updateScrollState]);
  const scrollByPage = (direction: -1 | 1) => {
    const strip = stripRef.current;
    if (!strip) return;
    strip.scrollBy({ left: direction * Math.max(240, strip.clientWidth * 0.75), behavior: 'smooth' });
  };

  return (
    <div className="sc-review-filmstrip-shell">
      <button
        className="sc-review-filmstrip-nav is-left"
        disabled={!scrollState.left}
        onClick={() => scrollByPage(-1)}
        aria-label={tx('previousPhotos')}
      >
        <ChevronLeft size={16} />
      </button>
      <div
        ref={stripRef}
        className="sc-review-filmstrip custom-scrollbar"
        aria-label={tx('groupPhotos')}
        onScroll={updateScrollState}
        onWheel={(event) => {
          if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) return;
          event.preventDefault();
          event.currentTarget.scrollLeft += event.deltaY;
        }}
      >
        {results.map((result) => {
          const comparisonSlot =
            result.resultId === focusedResultId ? 'A' : result.resultId === secondaryResultId ? 'B' : null;
          return (
            <article
              key={result.resultId}
              className={`sc-review-filmstrip-card ${comparisonSlot ? 'is-compared' : ''}`}
            >
              <button
                className="sc-review-filmstrip-preview"
                onClick={() => onSelect(result.resultId)}
                aria-label={`${tx('showInComparison')} ${fileName(result.path)}`}
              >
                <FilmstripPreview result={result} fallbackUrl={thumbnails[result.path]} />
                {comparisonSlot ? <i>{comparisonSlot}</i> : null}
              </button>
              <div className="sc-review-filmstrip-meta">
                <Stars
                  value={result.rating}
                  onChange={readOnly ? undefined : (rating) => onRating(result, rating)}
                  compact
                />
                <button
                  className={`sc-review-keep ${result.adopted ? 'is-checked' : ''}`}
                  disabled={readOnly}
                  onClick={() => onToggle(result)}
                  aria-label={result.adopted ? tx('doNotAdopt') : tx('adopt')}
                >
                  {result.adopted ? <Check size={11} /> : null}
                  {result.adopted ? tx('adopted') : tx('notAdopted')}
                </button>
                <span title={fileName(result.path)}>{fileName(result.path)}</span>
                <i className={`sc-color-dot is-${result.colorLabel ?? 'none'}`} />
              </div>
            </article>
          );
        })}
      </div>
      <button
        className="sc-review-filmstrip-nav is-right"
        disabled={!scrollState.right}
        onClick={() => scrollByPage(1)}
        aria-label={tx('nextPhotos')}
      >
        <ChevronRight size={16} />
      </button>
    </div>
  );
}
