import { Images, LockKeyhole, ShieldCheck, UserRoundSearch, X } from 'lucide-react';
import { useProcessStore } from '../../../store/useProcessStore';
import { SMART_CULLING_MODES, type SmartCullingMode } from '../constants';
import { useSmartCullingModes, useSmartCullingReasonText, useSmartCullingText } from '../i18n';
import type { ReviewResult } from '../types';
import { fileName } from './LifecycleChrome';
import { LabelControls, Stars } from './ReviewControls';
import { useRenderedPreview } from './useRenderedPreview';

export function ReviewInspector({
  result,
  onEdit,
  onModeChange,
  onEditGroupMode,
  onToggle,
  onOpenGroup,
  onSetComparison,
  readOnly = false,
  open = false,
  onClose,
}: {
  result: ReviewResult;
  onEdit: (patch: Partial<Pick<ReviewResult, 'rating' | 'colorLabel'>>) => void;
  onModeChange: (mode: SmartCullingMode) => void;
  onEditGroupMode: (mode: SmartCullingMode) => void;
  onToggle: () => void;
  onOpenGroup?: () => void;
  onSetComparison?: (slot: 'a' | 'b') => void;
  readOnly?: boolean;
  open?: boolean;
  onClose?: () => void;
}) {
  const tx = useSmartCullingText();
  const reason = useSmartCullingReasonText();
  const modeCopy = useSmartCullingModes();
  const thumbnail = useProcessStore((state) => state.thumbnails[result.path]);
  const { loadedUrl } = useRenderedPreview(result.path, thumbnail);
  const previewUrl = loadedUrl ?? thumbnail;
  const groupSummary =
    result.groupSize > 1
      ? `${result.rating}/5 ${tx('starRating')} · ${result.groupSize} ${tx('groupSuffix')}`
      : `${result.rating}/5 ${tx('starRating')} · ${tx('singlePhoto')}`;
  const sourceLabel = `${tx('source')}:`;
  const keyPersonEvidence = result.keyPersonEvidence.filter((evidence) => evidence.faceIndex !== null);
  return (
    <aside className={`sc-inspector ${open ? 'is-open' : ''}`}>
      <header>
        <div>
          <strong>{fileName(result.path)}</strong>
          <span>{groupSummary}</span>
        </div>
        <button className="sc-review-drawer-close" onClick={onClose} aria-label={tx('close')}>
          <X size={15} />
        </button>
      </header>
      <div className="sc-inspector-photo">
        {previewUrl ? <img src={previewUrl} alt={fileName(result.path)} /> : <div>{fileName(result.path)}</div>}
        <span>
          {result.width} × {result.height}
        </span>
      </div>
      <section>
        <h3>
          {tx('reviewEvidence')} <i>{result.source === 'ai' ? 'AI' : tx('manual')}</i>
        </h3>
        {result.source === 'manual' ? (
          <p className="sc-manual-reason">
            <LockKeyhole size={14} />
            {tx('manualReason')}
          </p>
        ) : (
          <>
            <p>{reason(result.reasonCodes)}</p>
            <p className="sc-review-evidence-hint">{tx('reviewEvidenceHint')}</p>
            {result.colorLabel === 'yellow' ? <p className="sc-review-human-check">{tx('needsHumanReview')}</p> : null}
            {result.faces.length > 0 ? <p className="sc-expression-pending">{tx('expressionPending')}</p> : null}
            {keyPersonEvidence.length > 0 ? (
              <div className="sc-key-person-evidence">
                <UserRoundSearch size={14} />
                <div>
                  {keyPersonEvidence.map((evidence) => (
                    <span key={evidence.priority}>
                      {tx('priority')} {evidence.priority} · {tx('keyPersonCandidate')}
                      {evidence.performanceRank ? ` · ${tx('performanceRank')} ${evidence.performanceRank}` : ''}
                    </span>
                  ))}
                  <span>{tx('identityPending')}</span>
                </div>
              </div>
            ) : null}
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
            onChange={(event) => onModeChange(event.target.value as SmartCullingMode)}
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
        {result.groupKind !== 'single' ? (
          <div className="sc-inspector-group-actions">
            <button onClick={onOpenGroup}>
              <Images size={13} />
              {result.groupKind === 'reviewOnly' ? tx('reviewOnly') : tx('similarGroup')}
            </button>
            {result.groupKind === 'similar' ? (
              <>
                <button onClick={() => onSetComparison?.('a')}>{tx('setAsA')}</button>
                <button onClick={() => onSetComparison?.('b')}>{tx('setAsB')}</button>
              </>
            ) : null}
          </div>
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
