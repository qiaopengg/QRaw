import type { CSSProperties } from 'react';
import { useCallback, useMemo, useState } from 'react';
import { List } from 'react-window';
import { buildReviewGalleryRows, type ReviewGalleryRow as GalleryRow } from '../reviewGalleryLayout';
import type { ReviewResult } from '../types';
import { ReviewResultCard } from './ReviewResultCard';

interface GalleryRowProps {
  rows: GalleryRow[];
  focusedResultId: string | null;
  onSelect: (result: ReviewResult) => void;
  onToggle: (result: ReviewResult) => void;
  onOpenGroup: (result: ReviewResult) => void;
  readOnly: boolean;
}

function GalleryRowView({
  index,
  style,
  rows,
  focusedResultId,
  onSelect,
  onToggle,
  onOpenGroup,
  readOnly,
}: GalleryRowProps & { index: number; style: CSSProperties }) {
  const row = rows[index];
  return (
    <div className="sc-gallery-row" style={style}>
      {row.items.map(({ result, width }) => (
        <ReviewResultCard
          key={result.resultId}
          result={result}
          width={width}
          imageHeight={row.imageHeight}
          selected={focusedResultId === result.resultId}
          onSelect={() => onSelect(result)}
          onToggle={() => onToggle(result)}
          onOpenGroup={result.groupKind === 'single' ? undefined : () => onOpenGroup(result)}
          readOnly={readOnly}
        />
      ))}
    </div>
  );
}

export function ReviewGallery({
  results,
  focusedResultId,
  onSelect,
  onToggle,
  onOpenGroup,
  onRequestThumbnails,
  readOnly,
}: {
  results: ReviewResult[];
  focusedResultId: string | null;
  onSelect: (result: ReviewResult) => void;
  onToggle: (result: ReviewResult) => void;
  onOpenGroup: (result: ReviewResult) => void;
  onRequestThumbnails?: (paths: string[]) => void;
  readOnly: boolean;
}) {
  const [width, setWidth] = useState(1000);
  const rows = useMemo(() => buildReviewGalleryRows(results, width - 28), [results, width]);
  const rowHeight = useCallback((index: number) => rows[index]?.height ?? 360, [rows]);

  return (
    <List<GalleryRowProps>
      className="sc-review-gallery custom-scrollbar"
      rowCount={rows.length}
      rowHeight={rowHeight}
      rowComponent={GalleryRowView}
      rowProps={{ rows, focusedResultId, onSelect, onToggle, onOpenGroup, readOnly }}
      overscanCount={2}
      onResize={({ width: nextWidth }) => setWidth(nextWidth)}
      onRowsRendered={(_, overscan) => {
        if (!onRequestThumbnails) return;
        const paths = rows
          .slice(overscan.startIndex, overscan.stopIndex + 1)
          .flatMap((row) => row.items.map((item) => item.result.path));
        if (paths.length > 0) onRequestThumbnails(paths);
      }}
      style={{ width: '100%', height: '100%' }}
    />
  );
}
