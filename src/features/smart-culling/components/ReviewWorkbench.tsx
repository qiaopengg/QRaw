import {
  AlertTriangle,
  ChevronDown,
  ChevronLeft,
  FileWarning,
  Folder,
  PanelLeftOpen,
  PanelRightOpen,
  Search,
  SearchX,
  SlidersHorizontal,
  X,
} from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useSmartCullingFailureText } from '../errorText';
import { useSmartCullingModes, useSmartCullingStoryText, useSmartCullingText } from '../i18n';
import type { ReviewChange, ReviewResult, SmartCullingSnapshot } from '../types';
import { runSmartCullingCommand, useSmartCullingStore } from '../useSmartCulling';
import { ConfirmModal } from './ConfirmModal';
import { LifecycleChrome, Modal } from './LifecycleChrome';
import { ReviewCompareWorkspace } from './ReviewCompareWorkspace';
import { ReviewInspector } from './ReviewInspector';
import { ReviewQueueNavigation } from './ReviewQueueNavigation';

export function ReviewWorkbench({ snapshot }: { snapshot: SmartCullingSnapshot }) {
  const tx = useSmartCullingText();
  const failureText = useSmartCullingFailureText();
  const modeCopy = useSmartCullingModes();
  const storyText = useSmartCullingStoryText();
  const readOnly = snapshot.state === 'completed';
  const { focusedResultId, confirmOpen, setState } = useSmartCullingStore();
  const [folder, setFolder] = useState<string>('all');
  const [story, setStory] = useState<string>('all');
  const [filter, setFilter] = useState<'all' | 'selected' | 'pending'>('all');
  const [showFailures, setShowFailures] = useState(false);
  const [query, setQuery] = useState('');
  const [navigationOpen, setNavigationOpen] = useState(false);
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [pendingGroupMode, setPendingGroupMode] = useState<ReviewResult['mode'] | null>(null);
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const folders = useMemo(
    () => Array.from(new Set(snapshot.results.map((result) => result.folder))),
    [snapshot.results],
  );
  const stories = useMemo(
    () =>
      Array.from(
        new Set(
          snapshot.results
            .filter((result) => folder === 'all' || result.folder === folder)
            .map((result) => result.story),
        ),
      ),
    [snapshot.results, folder],
  );
  const visible = useMemo(
    () =>
      snapshot.results.filter(
        (result) =>
          (folder === 'all' || result.folder === folder) &&
          (story === 'all' || result.story === story) &&
          (!normalizedQuery ||
            result.folder.toLocaleLowerCase().includes(normalizedQuery) ||
            storyText(result.story).toLocaleLowerCase().includes(normalizedQuery) ||
            result.path.toLocaleLowerCase().includes(normalizedQuery)) &&
          (filter === 'all' ||
            (filter === 'selected' && result.adopted) ||
            (filter === 'pending' && result.colorLabel === 'yellow')),
      ),
    [snapshot.results, folder, story, filter, normalizedQuery, storyText],
  );
  const groups = useMemo(() => {
    const visibleGroupIds = new Set(visible.map((result) => result.groupId));
    const map = new Map<string, ReviewResult[]>();
    snapshot.results
      .filter((result) => visibleGroupIds.has(result.groupId))
      .forEach((result) => map.set(result.groupId, [...(map.get(result.groupId) ?? []), result]));
    return [...map.entries()];
  }, [snapshot.results, visible]);
  const visibleGroupIds = useMemo(() => new Set(groups.map(([groupId]) => groupId)), [groups]);
  const focused =
    snapshot.results.find((result) => result.resultId === focusedResultId && visibleGroupIds.has(result.groupId)) ??
    visible[0] ??
    null;
  const focusedGroup = focused ? (groups.find(([groupId]) => groupId === focused.groupId)?.[1] ?? [focused]) : [];
  const picked = snapshot.results.filter((result) => result.colorLabel === 'green').length;
  const pending = snapshot.results.filter((result) => result.colorLabel === 'yellow').length;
  const rejected = snapshot.results.filter((result) => result.colorLabel === 'red').length;
  const hasFailures = snapshot.failures.length > 0;
  const update = (changes: ReviewChange[]) =>
    readOnly
      ? Promise.resolve(snapshot)
      : runSmartCullingCommand({ action: 'updateReview', changes }, true).catch(() => snapshot);
  const changeFor = (result: ReviewResult, patch: Partial<ReviewResult>, edited: boolean): ReviewChange => ({
    resultId: result.resultId,
    adopted: patch.adopted ?? result.adopted,
    rating: patch.rating ?? result.rating,
    colorLabel: patch.colorLabel === undefined ? result.colorLabel : patch.colorLabel,
    mode: patch.mode ?? result.mode,
    edited,
  });
  const editFocused = (patch: Partial<Pick<ReviewResult, 'rating' | 'colorLabel' | 'mode'>>) =>
    focused && void update([changeFor(focused, patch, true)]);
  const editResult = (result: ReviewResult, patch: Partial<Pick<ReviewResult, 'rating' | 'colorLabel' | 'mode'>>) =>
    void update([changeFor(result, patch, true)]);
  const editFocusedGroupMode = (mode: ReviewResult['mode']) =>
    focused &&
    void update(
      snapshot.results
        .filter((result) => result.groupId === focused.groupId)
        .map((result) => changeFor(result, { mode }, true)),
    );
  const toggle = (result: ReviewResult) => void update([changeFor(result, { adopted: !result.adopted }, false)]);
  const setAll = (adopted: boolean) =>
    void update(snapshot.results.map((result) => changeFor(result, { adopted }, false)));
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (
        event.metaKey ||
        event.ctrlKey ||
        event.altKey ||
        target?.matches('input, textarea, select, [contenteditable="true"]')
      ) {
        return;
      }
      const currentIndex = focused ? visible.findIndex((result) => result.resultId === focused.resultId) : -1;
      if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
        if (visible.length === 0) return;
        event.preventDefault();
        const direction = event.key === 'ArrowLeft' ? -1 : 1;
        const nextIndex = Math.min(visible.length - 1, Math.max(0, currentIndex + direction));
        setState({ focusedResultId: visible[nextIndex].resultId });
        return;
      }
      if (!focused || readOnly) return;
      if (/^[1-5]$/.test(event.key)) {
        event.preventDefault();
        editFocused({ rating: Number(event.key) });
      } else if (event.key === ' ') {
        event.preventDefault();
        toggle(focused);
      } else if (['g', 'y', 'r'].includes(event.key.toLowerCase())) {
        event.preventDefault();
        const labels = { g: 'green', y: 'yellow', r: 'red' } as const;
        editFocused({ colorLabel: labels[event.key.toLowerCase() as keyof typeof labels] });
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [editFocused, focused, readOnly, setState, toggle, visible]);
  return (
    <div className="sc-page sc-review-page">
      <LifecycleChrome screen="review">
        <div className="sc-review-counts">
          <span>
            {tx('pickedCount')} {picked}
          </span>
          <span>
            {tx('pendingCount')} {pending}
          </span>
          <span>
            {tx('rejectedCount')} {rejected}
          </span>
          <span>
            {tx('failureCount')} {snapshot.failures.length}
          </span>
        </div>
        <button
          className="sc-primary sc-apply"
          onClick={() =>
            readOnly
              ? setState({ screen: 'write' })
              : snapshot.results.some((result) => result.adopted)
                ? setState({ confirmOpen: true })
                : setState({ abandonOpen: true })
          }
        >
          {readOnly ? tx('back') : tx('confirmApply')}{' '}
          {readOnly ? null : snapshot.results.filter((result) => result.adopted).length}
        </button>
      </LifecycleChrome>
      <main className="sc-review-layout">
        <aside className={`sc-story-sidebar ${navigationOpen ? 'is-open' : ''}`}>
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
              aria-label={tx('searchPlaceholder')}
            />
          </div>
          <h3>
            <Folder size={13} />
            {tx('folders')}
          </h3>
          <button
            className={folder === 'all' ? 'is-active' : ''}
            onClick={() => {
              setFolder('all');
              setStory('all');
            }}
          >
            <span>{tx('allFolders')}</span>
            <em>{snapshot.results.length}</em>
          </button>
          {folders
            .filter((item) => !normalizedQuery || item.toLocaleLowerCase().includes(normalizedQuery))
            .map((item) => (
              <button
                key={item}
                className={folder === item ? 'is-active' : ''}
                onClick={() => {
                  setFolder(item);
                  setStory('all');
                }}
              >
                <span>{item}</span>
                <em>{snapshot.results.filter((result) => result.folder === item).length}</em>
              </button>
            ))}
          <h3>
            <SlidersHorizontal size={13} />
            {tx('stories')}
          </h3>
          {stories
            .filter((item) => !normalizedQuery || storyText(item).toLocaleLowerCase().includes(normalizedQuery))
            .map((item) => (
              <button key={item} className={story === item ? 'is-active' : ''} onClick={() => setStory(item)}>
                <span>{storyText(item)}</span>
                <em>{snapshot.results.filter((result) => result.story === item).length}</em>
              </button>
            ))}
          <ReviewQueueNavigation
            groups={groups}
            focusedGroupId={focused?.groupId ?? null}
            onSelect={(resultId) => {
              setState({ focusedResultId: resultId });
              setNavigationOpen(false);
            }}
          />
          <footer>
            <span>
              {tx('selectedCount')} {snapshot.results.filter((result) => result.adopted).length}
            </span>
            <button disabled={readOnly} onClick={() => setAll(true)}>
              {tx('selectAll')}
            </button>
            <button disabled={readOnly} onClick={() => setAll(false)}>
              {tx('clearAll')}
            </button>
          </footer>
        </aside>
        <section className="sc-review-canvas">
          <header>
            <button
              className="sc-review-panel-button"
              onClick={() => setNavigationOpen(true)}
              aria-label={tx('folders')}
            >
              <PanelLeftOpen size={16} />
            </button>
            <div>
              <h1>{folder === 'all' ? tx('allFolders') : folder}</h1>
              <span>{story === 'all' ? visible.length : storyText(story)}</span>
            </div>
            <nav>
              <button className={filter === 'all' ? 'is-active' : ''} onClick={() => setFilter('all')}>
                {tx('all')}
              </button>
              <button className={filter === 'selected' ? 'is-active' : ''} onClick={() => setFilter('selected')}>
                {tx('selected')}
              </button>
              <button className={filter === 'pending' ? 'is-active' : ''} onClick={() => setFilter('pending')}>
                {tx('pending')}
              </button>
            </nav>
            <button
              className="sc-review-panel-button"
              onClick={() => setInspectorOpen(true)}
              aria-label={tx('aiReason')}
            >
              <PanelRightOpen size={16} />
            </button>
          </header>
          {focused ? (
            <ReviewCompareWorkspace
              results={focusedGroup}
              focusedResultId={focused.resultId}
              onFocus={(resultId) => setState({ focusedResultId: resultId })}
              onToggle={toggle}
              onRating={(result, rating) => editResult(result, { rating })}
              onLabel={(result, colorLabel) => editResult(result, { colorLabel })}
              onOpenInspector={() => setInspectorOpen(true)}
              readOnly={readOnly}
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
                  setStory('all');
                  setFilter('all');
                  setQuery('');
                }}
              >
                {tx('clearFilters')}
              </button>
            </section>
          )}
          <section className="sc-failures">
            <button
              disabled={!hasFailures}
              aria-expanded={hasFailures ? showFailures : false}
              onClick={() => setShowFailures(!showFailures)}
            >
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
            result={focused}
            onEdit={editFocused}
            onEditGroupMode={setPendingGroupMode}
            onToggle={() => toggle(focused)}
            readOnly={readOnly}
            open={inspectorOpen}
            onClose={() => setInspectorOpen(false)}
          />
        ) : null}
      </main>
      {pendingGroupMode && focused ? (
        <Modal onClose={() => setPendingGroupMode(null)}>
          <span className="sc-dialog-icon warning">
            <AlertTriangle size={22} />
          </span>
          <h2>{tx('applyModeTitle')}</h2>
          <p>{tx('applyModeBody')}</p>
          <footer>
            <button className="sc-secondary" onClick={() => setPendingGroupMode(null)}>
              {tx('cancel')}
            </button>
            <button
              className="sc-primary"
              onClick={() => {
                editFocusedGroupMode(pendingGroupMode);
                setPendingGroupMode(null);
              }}
            >
              {tx('applyModeConfirm')}
            </button>
          </footer>
        </Modal>
      ) : null}
      {confirmOpen && !readOnly ? <ConfirmModal snapshot={snapshot} /> : null}
    </div>
  );
}
