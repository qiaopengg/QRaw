import { Star } from 'lucide-react';
import { useSmartCullingText } from '../i18n';
import type { ReviewResult } from '../types';

export function Stars({
  value,
  onChange,
  compact = false,
}: {
  value: number;
  onChange?: (value: number) => void;
  compact?: boolean;
}) {
  const tx = useSmartCullingText();
  return (
    <div className={`sc-stars ${compact ? 'is-compact' : ''}`}>
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
          <Star size={compact ? 12 : 16} className={rating <= value ? 'fill-current' : ''} />
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
