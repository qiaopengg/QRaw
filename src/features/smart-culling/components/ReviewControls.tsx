import { Star } from 'lucide-react';
import { useSmartCullingText } from '../i18n';
import { reviewResultNeedsAttention } from '../reviewPolicy';
import type { ReviewResult } from '../types';

export function Stars({
  result,
  onChange,
  compact = false,
}: {
  result: ReviewResult;
  onChange?: (value: number) => void;
  compact?: boolean;
}) {
  const tx = useSmartCullingText();
  const unrated = reviewResultNeedsAttention(result);
  const value = unrated ? null : result.rating;
  if (unrated && !onChange) return <span className="sc-unrated">{tx('unrated')}</span>;
  return (
    <div
      className={`sc-stars ${compact ? 'is-compact' : ''}`}
      role="group"
      aria-label={unrated ? tx('unrated') : `${value} ${tx('starRating')}`}
    >
      {unrated ? <span className="sc-unrated">{tx('unrated')}</span> : null}
      {onChange ? (
        <button
          aria-label={`0 ${tx('starRating')}`}
          onClick={(event) => {
            event.stopPropagation();
            onChange(0);
          }}
        >
          0
        </button>
      ) : value === 0 ? (
        <span>0</span>
      ) : null}
      {[1, 2, 3, 4, 5].map((rating) => (
        <button
          key={rating}
          disabled={!onChange}
          onClick={(event) => {
            event.stopPropagation();
            onChange?.(value === rating ? 0 : rating);
          }}
          aria-label={`${rating} ${tx('starRating')}`}
        >
          <Star size={compact ? 12 : 16} className={value !== null && rating <= value ? 'fill-current' : ''} />
        </button>
      ))}
    </div>
  );
}

export function LabelControls({
  value,
  onChange,
  disabled = false,
}: {
  value: ReviewResult['colorLabel'];
  onChange: (value: ReviewResult['colorLabel']) => void;
  disabled?: boolean;
}) {
  const tx = useSmartCullingText();
  return (
    <div className="sc-label-controls">
      {(['green', 'yellow', 'red'] as const).map((color) => (
        <button
          key={color}
          disabled={disabled}
          className={value === color ? `is-active is-${color}` : ''}
          onClick={() => onChange(value === color ? null : color)}
        >
          {tx(color)}
        </button>
      ))}
    </div>
  );
}
