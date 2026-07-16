import { Check, LockKeyhole, Star } from 'lucide-react';
import { useSmartCullingReasonText, useSmartCullingText } from '../i18n';
import type { ReviewResult } from '../types';
import { fileName } from './LifecycleChrome';

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

export function ReviewPhotoCard({
  result,
  thumbnail,
  focused,
  onFocus,
  onToggle,
  onRating,
  readOnly = false,
}: {
  result: ReviewResult;
  thumbnail?: string;
  focused: boolean;
  onFocus: () => void;
  onToggle: () => void;
  onRating: (rating: number) => void;
  readOnly?: boolean;
}) {
  const tx = useSmartCullingText();
  const reason = useSmartCullingReasonText();
  return (
    <article
      className={`sc-photo-card ${focused ? 'is-focused' : ''} ${result.source === 'manual' ? 'is-manual' : ''}`}
      onClick={onFocus}
    >
      <div className="sc-photo-media">
        {thumbnail ? <img src={thumbnail} alt={fileName(result.path)} /> : <div>{fileName(result.path)}</div>}
        <button
          className={`sc-keep-check ${result.adopted ? 'is-checked' : ''}`}
          disabled={readOnly}
          aria-label={result.adopted ? tx('doNotAdopt') : tx('adopt')}
          onClick={(event) => {
            event.stopPropagation();
            onToggle();
          }}
        >
          {result.adopted ? <Check size={13} /> : null}
        </button>
        {result.source === 'manual' ? (
          <span className="sc-manual-lock">
            <LockKeyhole size={10} />
            {tx('manualShort')}
          </span>
        ) : null}
      </div>
      <div className="sc-photo-meta">
        <div>
          <Stars value={result.rating} onChange={readOnly ? undefined : onRating} compact />
          <span className={`sc-label is-${result.colorLabel ?? 'none'}`}>
            {result.colorLabel ? tx(result.colorLabel) : tx('noLabel')}
          </span>
        </div>
        <div>
          <span>{fileName(result.path)}</span>
          <i>{result.source === 'ai' ? tx('ai') : tx('manualShort')}</i>
        </div>
        <small>{result.source === 'ai' ? reason(result.reasonCodes) : tx('manualProtected')}</small>
      </div>
    </article>
  );
}
