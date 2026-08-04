import {
  ChevronDown,
  ChevronLeft,
  FileWarning,
  Folder,
  LockKeyhole,
  PanelLeftOpen,
  PanelRightOpen,
  Search,
  SearchX,
  X,
} from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { toast } from 'react-toastify';
import { useSmartCullingFailureText } from '../errorText';
import { useSmartCullingModes, useSmartCullingText } from '../i18n';
import { reviewResultIsWritable, reviewResultNeedsAttention } from '../reviewPolicy';
import type { ReviewChange, ReviewResult, SmartCullingSnapshot } from '../types';
import { runSmartCullingCommand, useSmartCullingStore } from '../useSmartCulling';
import { ConfirmModal } from './ConfirmModal';
import { LifecycleChrome, Modal } from './LifecycleChrome';
import { ReviewCompareDialog } from './ReviewCompareDialog';
import { ReviewGallery } from './ReviewGallery';
import { ReviewInspector } from './ReviewInspector';
import { ReviewQueueNavigation } from './ReviewQueueNavigation';
import { SimilarGroupReview } from './SimilarGroupReview';
import { useDrawerFocus } from './useDrawerFocus';

type ReviewFilter = 'all' | 'pending' | 'manual';
type CompareSlots = { groupId: string | null; a: string | null; b: string | null };
type PendingMetadataEdit = {
  result: ReviewResult;
  patch: Partial<Pick<ReviewResult, 'rating' | 'colorLabel'>>;
};

export function ReviewWorkbench({
  snapshot,
  onRequestThumbnails,
}: {
  snapshot: SmartCullingSnapshot;
  onRequestThumbnails?: (paths: string[]) => void;
}) {
  const tx = useSmartCullingText();
  const failureText = useSmartCullingFailureText();
  const modeCopy = useSmartCullingModes();
  const readOnly = snapshot.state === 'completed';
  const { focusedResultId, confirmOpen, manualSyncPending, setState } = useSmartCullingStore();
  const [folder, setFolder] = useState('all');
  const [filter, setFilter] = useState<ReviewFilter>(() =>
    snapshot.results.some(reviewResultNeedsAttention) ? 'pending' : 'all',
  );
  const [query, setQuery] = useState('');
  const [navigationOpen, setNavigationOpen] = useState(false);
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [showFailures, setShowFailures] = useState(false);
  const [activeGroupId, setActiveGroupId] = useState<string | null>(null);
  const [compareSlots, setCompareSlots] = useState<CompareSlots>({ groupId: null, a: null, b: null });
  const [compareOpen, setCompareOpen] = useState(false);
  const [pendingMetadataEdit, setPendingMetadataEdit] = useState<PendingMetadataEdit | null>(null);
  const [manualEditAcknowledged, setManualEditAcknowledged] = useState(false);
  const navigationRef = useDrawerFocus(navigationOpen, () => setNavigationOpen(false));
  const inspectorRef = useDrawerFocus(inspectorOpen, () => setInspectorOpen(false));
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const folders = useMemo(
    () => Array.from(new Set(snapshot.results.map((result) => result.folder))),
    [snapshot.results],
  );
  const visible = useMemo(
    () =>
      snapshot.results.filter((result) => {
        const matchesFolder = folder === 'all' || result.folder === folder;
        const matchesQuery =
          !normalizedQuery ||
          result.folder.toLocaleLowerCase().includes(normalizedQuery) ||
          result.path.toLocaleLowerCase().includes(normalizedQuery);
        const matchesFilter =
          filter === 'all' ||
          (filter === 'manual' && result.source === 'manual') ||
          (filter === 'pending' && reviewResultNeedsAttention(result));
        return matchesFolder && matchesQuery && matchesFilter;
      }),
    [filter, folder, normalizedQuery, snapshot.results],
  );
  const navigationGroups = useMemo(() => {
    const visibleGroupIds = new Set(visible.map((result) => result.groupId));
    const map = new Map<string, ReviewResult[]>();
    snapshot.results
      .filter((result) => visibleGroupIds.has(result.groupId))
      .forEach((result) => map.set(result.groupId, [...(map.get(result.groupId) ?? []), result]));
    return [...map.entries()];
  }, [snapshot.results, visible]);
  const focused = snapshot.results.find((result) => result.resultId === focusedResultId) ?? visible[0] ?? null;
  const activeGroup = activeGroupId ? snapshot.results.filter((result) => result.groupId === activeGroupId) : [];
  const compareA = snapshot.results.find((result) => result.resultId === compareSlots.a) ?? null;
  const compareB = snapshot.results.find((result) => result.resultId === compareSlots.b) ?? null;
  const writable = snapshot.results.filter(reviewResultIsWritable).length;
  const pendingQueue = snapshot.results.filter(reviewResultNeedsAttention);
  const pending = pendingQueue.length;
  const pendingPosition = focused
    ? Math.max(0, pendingQueue.findIndex((result) => result.resultId === focused.resultId) + 1)
    : 0;
  const hasFailures = snapshot.failures.length > 0;

  const update = async (changes: ReviewChange[]) => {
    if (readOnly) return snapshot;
    try {
      return await runSmartCullingCommand({ action: 'updateReview', changes }, true);
    } catch {
      toast.error(tx('reviewUpdateFailed'));
      return snapshot;
    }
  };
  const changeFor = (
    result: ReviewResult,
    patch: Partial<Pick<ReviewResult, 'rating' | 'colorLabel'>>,
  ): ReviewChange => ({
    resultId: result.resultId,
    rating: patch.rating ?? result.rating,
    colorLabel: patch.colorLabel === undefined ? result.colorLabel : patch.colorLabel,
  });
  const commitMetadataEdit = (result: ReviewResult, patch: Partial<Pick<ReviewResult, 'rating' | 'colorLabel'>>) =>
    void update([changeFor(result, patch)]);
  const editMetadata = (result: ReviewResult, patch: Partial<Pick<ReviewResult, 'rating' | 'colorLabel'>>) => {
    if (result.source === 'ai' && !manualEditAcknowledged) {
      setPendingMetadataEdit({ result, patch });
      return;
    }
    commitMetadataEdit(result, patch);
  };
  const setComparison = (slot: 'a' | 'b', result: ReviewResult) => {
    const sameGroup = compareSlots.groupId === result.groupId;
    const next = {
      groupId: result.groupId,
      a: sameGroup ? compareSlots.a : null,
      b: sameGroup ? compareSlots.b : null,
      [slot]: result.resultId,
    };
    setCompareSlots(next);
    const other = slot === 'a' ? next.b : next.a;
    if (other && other !== result.resultId) setCompareOpen(true);
  };
  const filterText = (item: ReviewFilter) => (item === 'manual' ? tx('manualFilter') : tx(item));
  const goToNextPending = () => {
    if (pendingQueue.length === 0) return;
    const currentIndex = pendingQueue.findIndex((result) => result.resultId === focused?.resultId);
    const next = pendingQueue[currentIndex < 0 ? 0 : (currentIndex + 1) % pendingQueue.length];
    setFolder('all');
    setFilter('pending');
    setQuery('');
    setState({ focusedResultId: next.resultId });
    setInspectorOpen(false);
  };

  useEffect(() => {
    const focusedIsVisible = visible.some((result) => result.resultId === focusedResultId);
    if (!activeGroupId && visible[0] && !focusedIsVisible) setState({ focusedResultId: visible[0].resultId });
  }, [activeGroupId, focusedResultId, setState, visible]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (pendingMetadataEdit) return;
      const target = event.target as HTMLElement | null;
      if (
        event.metaKey ||
        event.ctrlKey ||
        event.altKey ||
        target?.matches('input, textarea, select, [contenteditable="true"]')
      )
        return;
      const currentIndex = focused ? visible.findIndex((result) => result.resultId === focused.resultId) : -1;
      if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
        if (visible.length === 0) return;
        event.preventDefault();
        const direction = event.key === 'ArrowLeft' ? -1 : 1;
        const nextIndex = Math.min(visible.length - 1, Math.max(0, currentIndex + direction));
        setState({ focusedResultId: visible[nextIndex].resultId });
      } else if (focused && !readOnly && /^[1-5]$/.test(event.key)) {
        event.preventDefault();
        editMetadata(focused, { rating: Number(event.key) });
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [focused, pendingMetadataEdit, readOnly, setState, visible]);

  return (
    <div className="sc-page sc-review-page">
      <LifecycleChrome screen="review">
        <div className="sc-review-counts">
          <span>
            {tx('pendingCount')} {pending}
          </span>
          <span>
            {tx('writeableCount')} {writable}
          </span>
        </div>
        <button
          className="sc-primary sc-apply"
          onClick={() =>
            readOnly
              ? setState({ screen: 'write' })
              : writable > 0 && !manualSyncPending
                ? setState({ confirmOpen: true })
                : undefined
          }
          disabled={!readOnly && (writable === 0 || manualSyncPending)}
        >
          {readOnly ? tx('back') : tx('confirmApply')} {readOnly ? null : writable}
        </button>
      </LifecycleChrome>
      {manualSyncPending ? (
        <div className="sc-manual-sync-notice" role="status">
          <LockKeyhole size={16} />
          <span>{tx('manualSyncPending')}</span>
        </div>
      ) : null}
      {snapshot.progress.partial ? (
        <div className="sc-partial-review-notice" role="status">
          <FileWarning size={16} />
          <span>
            {tx('partialReviewPrefix')} {snapshot.progress.completed.toLocaleString()} /{' '}
            {snapshot.progress.total.toLocaleString()} · {tx('partialReviewSuffix')}
          </span>
        </div>
      ) : null}
      <main className="sc-review-layout">
        {navigationOpen || inspectorOpen ? (
          <div
            className="sc-review-drawer-backdrop"
            aria-hidden="true"
            onMouseDown={() => {
              setNavigationOpen(false);
              setInspectorOpen(false);
            }}
          />
        ) : null}
        <aside
          ref={navigationRef}
          className={`sc-review-sidebar ${navigationOpen ? 'is-open' : ''}`}
          role="dialog"
          aria-modal={navigationOpen || undefined}
          aria-label={tx('folders')}
          aria-hidden={!navigationOpen}
          tabIndex={-1}
        >
          <header>
            <button onClick={() => setState({ abandonOpen: true })} aria-label={tx('back')}>
              <ChevronLeft size={15} />
            </button>
            <strong>{tx('reviewTitle')}</strong>
            <span>
              {snapshot.mode ? modeCopy[snapshot.mode][0] : tx('title')} · {snapshot.results.length}
            </span>
            <button
              className="sc-review-drawer-close"
              onClick={() => setNavigationOpen(false)}
              aria-label={tx('close')}
            >
              <X size={15} />
            </button>
          </header>
          <div className="sc-search">
            <Search size={14} />
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={tx('searchPlaceholder')}
            />
          </div>
          <h3>
            <Folder size={13} />
            {tx('folders')}
          </h3>
          <button className={folder === 'all' ? 'is-active' : ''} onClick={() => setFolder('all')}>
            <span>{tx('allFolders')}</span>
            <em>{snapshot.results.length}</em>
          </button>
          {folders
            .filter((item) => !normalizedQuery || item.toLocaleLowerCase().includes(normalizedQuery))
            .map((item) => (
              <button key={item} className={folder === item ? 'is-active' : ''} onClick={() => setFolder(item)}>
                <span>{item}</span>
                <em>{snapshot.results.filter((result) => result.folder === item).length}</em>
              </button>
            ))}
          <ReviewQueueNavigation
            groups={navigationGroups}
            focusedGroupId={focused?.groupId ?? null}
            onSelect={(resultId) => {
              const result = snapshot.results.find((item) => item.resultId === resultId);
              if (result?.groupKind !== 'single') setActiveGroupId(result?.groupId ?? null);
              setState({ focusedResultId: resultId });
              setNavigationOpen(false);
            }}
          />
          <footer>
            <span>
              {tx('writeableCount')} {writable}
            </span>
          </footer>
        </aside>
        <section className="sc-review-canvas">
          <header>
            <button
              className="sc-review-panel-button"
              onClick={() => {
                setInspectorOpen(false);
                setNavigationOpen(true);
              }}
              aria-label={tx('folders')}
            >
              <PanelLeftOpen size={16} />
            </button>
            <div className="sc-review-queue-heading-main">
              <span>{tx('reviewQueueTitle')}</span>
              <h1>{folder === 'all' ? tx('allResults') : folder}</h1>
              <span>
                {visible.length} {tx('photoUnit')}
              </span>
            </div>
            <div className="sc-review-next-control">
              <span>{pending ? `${tx('pending')} ${pendingPosition || 1} / ${pending}` : tx('noPending')}</span>
              <button className="sc-secondary" disabled={pending === 0} onClick={goToNextPending}>
                {tx('nextPending')}
              </button>
            </div>
            <nav>
              {(['all', 'pending', 'manual'] as ReviewFilter[]).map((item) => (
                <button key={item} className={filter === item ? 'is-active' : ''} onClick={() => setFilter(item)}>
                  {filterText(item)}
                </button>
              ))}
            </nav>
            <button
              className="sc-review-compare-button"
              disabled={!compareA || !compareB || compareA.resultId === compareB.resultId}
              title={!compareA || !compareB ? tx('compareNeedsTwo') : tx('compareSelected')}
              onClick={() => setCompareOpen(true)}
            >
              {tx('compareSelected')}
            </button>
            <button
              className="sc-review-panel-button"
              onClick={() => {
                setNavigationOpen(false);
                setInspectorOpen(true);
              }}
              aria-label={tx('reviewEvidence')}
            >
              <PanelRightOpen size={16} />
            </button>
          </header>
          <div className="sc-review-content">
            {visible.length > 0 ? (
              <ReviewGallery
                results={visible}
                focusedResultId={focused?.resultId ?? null}
                onSelect={(result) => {
                  setState({ focusedResultId: result.resultId });
                  setNavigationOpen(false);
                  setInspectorOpen(true);
                }}
                onOpenGroup={(result) => {
                  setActiveGroupId(result.groupId);
                  setState({ focusedResultId: result.resultId });
                }}
                onRequestThumbnails={onRequestThumbnails}
              />
            ) : (
              <section className="sc-review-empty" role="status">
                <SearchX size={24} />
                <h2>{tx('emptyReviewTitle')}</h2>
                <p>{tx('emptyReviewHint')}</p>
                <button
                  className="sc-secondary"
                  onClick={() => {
                    setFolder('all');
                    setFilter('all');
                    setQuery('');
                  }}
                >
                  {tx('clearFilters')}
                </button>
              </section>
            )}
            {activeGroup.length > 0 ? (
              <div className="sc-group-review-layer">
                <SimilarGroupReview
                  results={activeGroup}
                  focusedResultId={focused?.resultId ?? null}
                  compareAId={compareA?.resultId ?? null}
                  compareBId={compareB?.resultId ?? null}
                  onBack={() => setActiveGroupId(null)}
                  onSelect={(result) => {
                    setState({ focusedResultId: result.resultId });
                    setNavigationOpen(false);
                    setInspectorOpen(true);
                  }}
                  onSetComparison={setComparison}
                  onRequestThumbnails={onRequestThumbnails}
                />
              </div>
            ) : null}
          </div>
          <section className="sc-failures">
            <button disabled={!hasFailures} aria-expanded={showFailures} onClick={() => setShowFailures(!showFailures)}>
              <FileWarning size={16} />
              <strong>
                {tx('failureAndSkip')} · {snapshot.failures.length}
              </strong>
              <span>
                {snapshot.inventory.protectedAssets} {tx('protectedScanSummary')} · {snapshot.inventory.failedAssets}{' '}
                {tx('scanFailures')}
              </span>
              <ChevronDown className={showFailures ? 'is-open' : ''} size={15} />
            </button>
            {showFailures && hasFailures ? (
              <div>
                {snapshot.failures.slice(0, 100).map((failure, index) => (
                  <p key={`${failure.path}-${failure.code}-${index}`}>
                    <span>{failureText(failure)}</span>
                    <em>{failure.path}</em>
                    {failure.memberPaths.length > 1 ? <small>{failure.memberPaths.join(' · ')}</small> : null}
                  </p>
                ))}
              </div>
            ) : null}
          </section>
        </section>
        {focused ? (
          <ReviewInspector
            ref={inspectorRef}
            result={focused}
            compareAId={compareA?.resultId ?? null}
            compareBId={compareB?.resultId ?? null}
            onEdit={(patch) => editMetadata(focused, patch)}
            onOpenGroup={() => setActiveGroupId(focused.groupId)}
            onSetComparison={(slot) => setComparison(slot, focused)}
            readOnly={readOnly}
            open={inspectorOpen}
            onClose={() => setInspectorOpen(false)}
          />
        ) : null}
      </main>
      {compareOpen && compareA && compareB && compareA.resultId !== compareB.resultId ? (
        <ReviewCompareDialog
          first={compareA}
          second={compareB}
          onClose={() => setCompareOpen(false)}
          onFocus={(resultId) => setState({ focusedResultId: resultId })}
          onRating={(result, rating) => editMetadata(result, { rating })}
          onLabel={(result, colorLabel) => editMetadata(result, { colorLabel })}
          onOpenInspector={() => {
            setCompareOpen(false);
            setInspectorOpen(true);
          }}
          readOnly={readOnly}
        />
      ) : null}
      {pendingMetadataEdit ? (
        <Modal onClose={() => setPendingMetadataEdit(null)}>
          <span className="sc-dialog-icon warning">
            <LockKeyhole size={22} />
          </span>
          <h2>{tx('manualEditTitle')}</h2>
          <p>{tx('manualEditBody')}</p>
          <footer>
            <button className="sc-secondary" onClick={() => setPendingMetadataEdit(null)}>
              {tx('cancel')}
            </button>
            <button
              className="sc-primary"
              onClick={() => {
                setManualEditAcknowledged(true);
                commitMetadataEdit(pendingMetadataEdit.result, pendingMetadataEdit.patch);
                setPendingMetadataEdit(null);
              }}
            >
              {tx('saveAsManual')}
            </button>
          </footer>
        </Modal>
      ) : null}
      {confirmOpen && !readOnly ? <ConfirmModal snapshot={snapshot} /> : null}
    </div>
  );
}
