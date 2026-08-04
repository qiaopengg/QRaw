import type { ReviewResult } from './types';

export interface ReviewGalleryItem {
  result: ReviewResult;
  width: number;
}

export interface ReviewGalleryRow {
  items: ReviewGalleryItem[];
  imageHeight: number;
  height: number;
}

const GAP = 12;
// Title, rating controls, lock/identity state, and a two-line reason at the
// feature's 11px fine-print size.
const CARD_META_HEIGHT = 112;
const TARGET_IMAGE_HEIGHT = 250;
const MIN_IMAGE_HEIGHT = 170;
const MAX_IMAGE_HEIGHT = 310;

export function buildReviewGalleryRows(results: ReviewResult[], availableWidth: number): ReviewGalleryRow[] {
  const width = Math.max(280, availableWidth);
  const rows: ReviewGalleryRow[] = [];
  let pending: ReviewResult[] = [];
  let aspectTotal = 0;

  const commit = (isLast: boolean) => {
    if (pending.length === 0) return;
    const gaps = GAP * Math.max(0, pending.length - 1);
    const justifiedHeight = (width - gaps) / Math.max(aspectTotal, 0.1);
    const imageHeight = isLast
      ? Math.min(TARGET_IMAGE_HEIGHT, justifiedHeight)
      : Math.min(MAX_IMAGE_HEIGHT, Math.max(MIN_IMAGE_HEIGHT, justifiedHeight));
    rows.push({
      imageHeight,
      height: imageHeight + CARD_META_HEIGHT + GAP,
      items: pending.map((result) => ({
        result,
        width: Math.max(120, (result.width / Math.max(result.height, 1)) * imageHeight),
      })),
    });
    pending = [];
    aspectTotal = 0;
  };

  for (const result of results) {
    pending.push(result);
    aspectTotal += Math.max(0.25, Math.min(4, result.width / Math.max(result.height, 1)));
    if (aspectTotal * TARGET_IMAGE_HEIGHT + GAP * (pending.length - 1) >= width) {
      commit(false);
    }
  }
  commit(true);
  return rows;
}
