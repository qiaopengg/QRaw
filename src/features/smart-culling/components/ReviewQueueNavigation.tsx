import { Image as ImageIcon, Images } from 'lucide-react';
import { type CSSProperties, useMemo } from 'react';
import { List } from 'react-window';
import { useSmartCullingStoryText, useSmartCullingText } from '../i18n';
import type { ReviewResult } from '../types';
import { fileName } from './LifecycleChrome';

type ReviewGroup = [string, ReviewResult[]];
const LIST_STYLE: CSSProperties = { width: '100%', height: '100%' };

type QueueEntry =
  | { kind: 'header'; label: string; count: number }
  | { kind: 'group'; groupId: string; results: ReviewResult[] }
  | { kind: 'single'; groupId: string; result: ReviewResult };

interface ReviewQueueRowProps {
  entries: QueueEntry[];
  focusedGroupId: string | null;
  onSelect: (resultId: string) => void;
}

function ReviewQueueRow({
  index,
  style,
  entries,
  focusedGroupId,
  onSelect,
}: ReviewQueueRowProps & { index: number; style: CSSProperties }) {
  const tx = useSmartCullingText();
  const storyText = useSmartCullingStoryText();
  const entry = entries[index];

  if (entry.kind === 'header') {
    return (
      <div className="sc-review-queue-heading" style={style}>
        <span>{entry.label}</span>
        <em>{entry.count}</em>
      </div>
    );
  }

  if (entry.kind === 'single') {
    return (
      <div className="sc-review-queue-row" style={style}>
        <button
          className={focusedGroupId === entry.groupId ? 'is-active' : ''}
          onClick={() => onSelect(entry.result.resultId)}
          aria-pressed={focusedGroupId === entry.groupId}
        >
          <ImageIcon size={13} />
          <span>{fileName(entry.result.path)}</span>
          <em>{entry.result.adopted ? tx('adopted') : ''}</em>
        </button>
      </div>
    );
  }

  const primary = entry.results.find((result) => result.adopted) ?? entry.results[0];
  return (
    <div className="sc-review-queue-row" style={style}>
      <button
        className={focusedGroupId === entry.groupId ? 'is-active' : ''}
        onClick={() => onSelect(primary.resultId)}
        aria-pressed={focusedGroupId === entry.groupId}
      >
        <Images size={13} />
        <span>{storyText(primary.story)}</span>
        <em>
          {entry.results.length} {tx('photoUnit')}
        </em>
      </button>
    </div>
  );
}

export function ReviewQueueNavigation({
  groups,
  focusedGroupId,
  onSelect,
}: {
  groups: ReviewGroup[];
  focusedGroupId: string | null;
  onSelect: (resultId: string) => void;
}) {
  const tx = useSmartCullingText();
  const entries = useMemo(() => {
    const comparisonGroups = groups.filter(([, results]) => results.length > 1);
    const singleGroups = groups.filter(([, results]) => results.length === 1);
    return [
      { kind: 'header', label: tx('similarGroups'), count: comparisonGroups.length } as const,
      ...comparisonGroups.map(([groupId, results]) => ({ kind: 'group', groupId, results }) as const),
      { kind: 'header', label: tx('singlePhotos'), count: singleGroups.length } as const,
      ...singleGroups.map(([groupId, results]) => ({ kind: 'single', groupId, result: results[0] }) as const),
    ];
  }, [groups, tx]);

  if (groups.length === 0) return null;
  return (
    <div className="sc-review-queue">
      <List<ReviewQueueRowProps>
        rowCount={entries.length}
        rowHeight={34}
        rowComponent={ReviewQueueRow}
        rowProps={{ entries, focusedGroupId, onSelect }}
        overscanCount={4}
        className="sc-review-queue-list custom-scrollbar"
        style={LIST_STYLE}
      />
    </div>
  );
}
