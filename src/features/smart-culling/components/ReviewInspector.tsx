import { Images, LockKeyhole, ShieldCheck, UserRoundSearch, X } from 'lucide-react';
import { forwardRef } from 'react';
import { useProcessStore } from '../../../store/useProcessStore';
import { keyPersonIdentityLabel, useSmartCullingModes, useSmartCullingReasonText, useSmartCullingText } from '../i18n';
import type { ReviewResult } from '../types';
import { fileName } from './LifecycleChrome';
import { PhotoEvidenceViewport } from './PhotoEvidenceViewport';
import { LabelControls, Stars } from './ReviewControls';

export const ReviewInspector = forwardRef<
  HTMLElement,
  {
    result: ReviewResult;
    onEdit: (patch: Partial<Pick<ReviewResult, 'rating' | 'colorLabel'>>) => void;
    onOpenGroup?: () => void;
    onSetComparison?: (slot: 'a' | 'b') => void;
    compareAId?: string | null;
    compareBId?: string | null;
    readOnly?: boolean;
    open?: boolean;
    onClose?: () => void;
  }
>(function ReviewInspector(
  {
    result,
    onEdit,
    onOpenGroup,
    onSetComparison,
    compareAId = null,
    compareBId = null,
    readOnly = false,
    open = false,
    onClose,
  },
  ref,
) {
  const tx = useSmartCullingText();
  const reason = useSmartCullingReasonText();
  const modeCopy = useSmartCullingModes();
  const thumbnail = useProcessStore((state) => state.thumbnails[result.path]);
  const groupSummary =
    result.groupSize > 1
      ? `${result.rating}/5 ${tx('starRating')} · ${result.groupSize} ${tx('groupSuffix')}`
      : `${result.rating}/5 ${tx('starRating')} · ${tx('singlePhoto')}`;
  const sourceLabel = `${tx('source')}:`;
  const keyPersonStatusText = (status: ReviewResult['keyPersonEvidence'][number]['status']) => {
    if (status === 'confirmed') return tx('keyPersonConfirmed');
    if (status === 'missing') return tx('keyPersonMissing');
    if (status === 'suspected') return tx('suspectedKeyPerson');
    if (status === 'ambiguous') return tx('keyPersonAmbiguous');
    return tx('keyPersonUnknown');
  };
  return (
    <aside
      ref={ref}
      className={`sc-inspector ${open ? 'is-open' : ''}`}
      role="dialog"
      aria-modal={open || undefined}
      aria-label={tx('reviewEvidence')}
      aria-hidden={!open}
      tabIndex={-1}
    >
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
        <PhotoEvidenceViewport
          path={result.path}
          fallbackUrl={thumbnail}
          alt={fileName(result.path)}
          faces={result.faces}
          compact
        />
        <span className="sc-inspector-dimensions">
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
            {result.requiresHumanReview ? <p className="sc-review-human-check">{tx('needsHumanReview')}</p> : null}
            {result.faces.length > 0 ? <p className="sc-expression-pending">{tx('expressionPending')}</p> : null}
            {result.keyPersonEvidence.length > 0 ? (
              <div className="sc-key-person-evidence">
                <UserRoundSearch size={14} />
                <div>
                  {result.keyPersonEvidence.map((evidence) => (
                    <span key={evidence.priority}>
                      {tx('person')} {keyPersonIdentityLabel(evidence.priority)} ·{' '}
                      {keyPersonStatusText(evidence.status)}
                      {evidence.performanceRank ? ` · ${tx('performanceRank')} ${evidence.performanceRank}` : ''}
                    </span>
                  ))}
                  {result.keyPersonEvidence.some((evidence) =>
                    ['suspected', 'ambiguous', 'unknown'].includes(evidence.status),
                  ) ? (
                    <span>{tx('identityPending')}</span>
                  ) : null}
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
        <div className="sc-mode-select">
          <span>{tx('shootingMode')}</span>
          <strong>{modeCopy[result.mode][0]}</strong>
        </div>
        {result.groupKind !== 'single' ? (
          <div className="sc-inspector-group-actions">
            <button onClick={onOpenGroup}>
              <Images size={13} />
              {result.groupKind === 'reviewOnly' ? tx('reviewOnly') : tx('similarGroup')}
            </button>
            {result.groupKind === 'similar' ? (
              <>
                <button
                  className={compareAId === result.resultId ? 'is-active' : ''}
                  aria-pressed={compareAId === result.resultId}
                  onClick={() => onSetComparison?.('a')}
                >
                  {compareAId === result.resultId ? `${tx('setAsA')} ✓` : tx('setAsA')}
                </button>
                <button
                  className={compareBId === result.resultId ? 'is-active' : ''}
                  aria-pressed={compareBId === result.resultId}
                  onClick={() => onSetComparison?.('b')}
                >
                  {compareBId === result.resultId ? `${tx('setAsB')} ✓` : tx('setAsB')}
                </button>
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
          <strong>{result.protected ? tx('lockedResult') : tx('protectedAfterEdit')}</strong>
          <p>{result.protected ? tx('lockedResultHint') : tx('protectedHint')}</p>
        </div>
      </section>
      <footer>
        <span>
          <i className={`sc-color-dot is-${result.colorLabel ?? 'none'}`} />
          {result.colorLabel ? tx(result.colorLabel) : '—'}
        </span>
      </footer>
    </aside>
  );
});
