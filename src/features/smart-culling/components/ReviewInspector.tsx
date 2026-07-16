import { LockKeyhole, ShieldCheck } from 'lucide-react';
import { useProcessStore } from '../../../store/useProcessStore';
import { SMART_CULLING_MODES, type SmartCullingMode } from '../constants';
import { useSmartCullingModes, useSmartCullingReasonText, useSmartCullingText } from '../i18n';
import type { ReviewResult } from '../types';
import { fileName } from './LifecycleChrome';
import { LabelControls, Stars } from './ReviewControls';

export function ReviewInspector({
  result,
  onEdit,
  onEditGroupMode,
  onToggle,
  readOnly = false,
}: {
  result: ReviewResult;
  onEdit: (patch: Partial<Pick<ReviewResult, 'rating' | 'colorLabel' | 'mode'>>) => void;
  onEditGroupMode: (mode: SmartCullingMode) => void;
  onToggle: () => void;
  readOnly?: boolean;
}) {
  const tx = useSmartCullingText();
  const reason = useSmartCullingReasonText();
  const modeCopy = useSmartCullingModes();
  const thumbnail = useProcessStore((state) => state.thumbnails[result.path]);
  const groupSummary = [result.rating, 5, result.groupSize, tx('groupSuffix')].join(' · ');
  const sourceLabel = `${tx('source')}:`;
  return (
    <aside className="sc-inspector">
      <header>
        <div>
          <strong>{fileName(result.path)}</strong>
          <span>{groupSummary}</span>
        </div>
      </header>
      <div className="sc-inspector-photo">
        {thumbnail ? <img src={thumbnail} alt={fileName(result.path)} /> : <div>{fileName(result.path)}</div>}
        <span>
          {result.width} × {result.height}
        </span>
      </div>
      <section>
        <h3>
          {tx('aiReason')} <i>{result.source === 'ai' ? 'AI' : tx('manual')}</i>
        </h3>
        {result.source === 'manual' ? (
          <p className="sc-manual-reason">
            <LockKeyhole size={14} />
            {tx('manualReason')}
          </p>
        ) : (
          <>
            <p>{reason(result.reasonCodes)}</p>
            <div className="sc-confidence">
              <span>{tx('confidence')}</span>
              <i>
                <b style={{ width: `${Math.round(result.confidence * 100)}%` }} />
              </i>
              <em>{Math.round(result.confidence * 100)}%</em>
            </div>
          </>
        )}
      </section>
      <section>
        <h3>{tx('ratingAndLabel')}</h3>
        <Stars value={result.rating} onChange={readOnly ? undefined : (rating) => onEdit({ rating })} />
        <LabelControls
          value={result.colorLabel}
          disabled={readOnly}
          onChange={(colorLabel) => onEdit({ colorLabel })}
        />
        <label className="sc-mode-select">
          {tx('shootingMode')}
          <select
            disabled={readOnly}
            value={result.mode}
            onChange={(event) => onEdit({ mode: event.target.value as SmartCullingMode })}
          >
            {SMART_CULLING_MODES.map((mode) => (
              <option key={mode} value={mode}>
                {modeCopy[mode][0]}
              </option>
            ))}
          </select>
        </label>
        {result.groupSize > 1 ? (
          <button className="sc-group-mode-button" disabled={readOnly} onClick={() => onEditGroupMode(result.mode)}>
            {tx('applyModeToGroup')}
          </button>
        ) : null}
        <div className="sc-source-line">
          {sourceLabel}
          <span className={result.source === 'manual' ? 'is-manual' : ''}>
            {result.source === 'ai' ? tx('ai') : tx('manual')}
          </span>
        </div>
      </section>
      <section className="sc-protection-note">
        <ShieldCheck size={18} />
        <div>
          <strong>{tx('protectedAfterEdit')}</strong>
          <p>{tx('protectedHint')}</p>
        </div>
      </section>
      <footer>
        <span>
          <i className={`sc-color-dot is-${result.colorLabel ?? 'none'}`} />
          {result.colorLabel ? tx(result.colorLabel) : '—'}
        </span>
        <button disabled={readOnly} onClick={onToggle}>
          {result.adopted ? tx('doNotAdopt') : tx('adopt')}
        </button>
      </footer>
    </aside>
  );
}
