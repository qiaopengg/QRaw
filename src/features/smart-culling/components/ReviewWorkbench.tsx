import { ChevronDown, ChevronLeft, FileWarning, Folder, Search, SlidersHorizontal } from 'lucide-react';
import { useMemo, useState } from 'react';
import { useProcessStore } from '../../../store/useProcessStore';
import { useSmartCullingFailureText } from '../errorText';
import { useSmartCullingModes, useSmartCullingStoryText, useSmartCullingText } from '../i18n';
import type { ReviewChange, ReviewResult, SmartCullingSnapshot } from '../types';
import { runSmartCullingCommand, useSmartCullingStore } from '../useSmartCulling';
import { ConfirmModal } from './ConfirmModal';
import { LifecycleChrome } from './LifecycleChrome';
import { ReviewInspector } from './ReviewInspector';
import { ReviewPhotoCard } from './ReviewControls';

export function ReviewWorkbench({ snapshot }: { snapshot: SmartCullingSnapshot }) {
  const tx = useSmartCullingText();
  const failureText = useSmartCullingFailureText();
  const modeCopy = useSmartCullingModes();
  const storyText = useSmartCullingStoryText();
  const readOnly = snapshot.state === 'completed';
  const thumbnails = useProcessStore((state) => state.thumbnails);
  const { focusedResultId, confirmOpen, setState } = useSmartCullingStore();
  const [folder, setFolder] = useState<string>('all');
  const [story, setStory] = useState<string>('all');
  const [filter, setFilter] = useState<'all' | 'selected' | 'pending'>('all');
  const [showFailures, setShowFailures] = useState(true);
  const [query, setQuery] = useState('');
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
    const map = new Map<string, ReviewResult[]>();
    visible.forEach((result) => map.set(result.groupId, [...(map.get(result.groupId) ?? []), result]));
    return [...map.entries()];
  }, [visible]);
  const focused =
    snapshot.results.find((result) => result.resultId === focusedResultId) ?? visible[0] ?? snapshot.results[0];
  const picked = snapshot.results.filter((result) => result.colorLabel === 'green').length;
  const pending = snapshot.results.filter((result) => result.colorLabel === 'yellow').length;
  const rejected = snapshot.results.filter((result) => result.colorLabel === 'red').length;
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
  return (
    <div className="sc-page">
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
        <aside className="sc-story-sidebar">
          <header>
            <button onClick={() => setState({ abandonOpen: true })} aria-label={tx('back')}>
              <ChevronLeft size={15} />
            </button>
            <strong>{tx('reviewTitle')}</strong>
            <span>
              {snapshot.mode ? modeCopy[snapshot.mode][0] : tx('title')} · {snapshot.results.length}
            </span>
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
          </header>
          {groups.map(([groupId, results]) => (
            <section className="sc-similar-group" key={groupId}>
              <header>
                <div>
                  <i />
                  <strong>
                    {storyText(results[0].story)} · {results.length}
                  </strong>
                  <small>
                    {tx('recommended')} {results[0].recommendedCount}
                  </small>
                </div>
                <span>{tx('similarityHigh')}</span>
              </header>
              <div className="sc-review-row">
                {results.map((result) => (
                  <ReviewPhotoCard
                    key={result.resultId}
                    result={result}
                    thumbnail={thumbnails[result.path]}
                    focused={focused?.resultId === result.resultId}
                    onFocus={() => setState({ focusedResultId: result.resultId })}
                    onToggle={() => toggle(result)}
                    onRating={(rating) => void update([changeFor(result, { rating }, true)])}
                    readOnly={readOnly}
                  />
                ))}
              </div>
            </section>
          ))}
          <section className="sc-failures">
            <button onClick={() => setShowFailures(!showFailures)}>
              <FileWarning size={16} />
              <strong>
                {tx('failureAndSkip')} · {snapshot.failures.length}
              </strong>
              <span>
                {snapshot.inventory.protectedAssets} {tx('protectedScanSummary')} · {snapshot.inventory.failedAssets}{' '}
                {tx('scanFailures')}
              </span>
              <ChevronDown size={15} />
            </button>
            {showFailures ? (
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
            onEditGroupMode={editFocusedGroupMode}
            onToggle={() => toggle(focused)}
            readOnly={readOnly}
          />
        ) : null}
      </main>
      {confirmOpen && !readOnly ? <ConfirmModal snapshot={snapshot} /> : null}
    </div>
  );
}
