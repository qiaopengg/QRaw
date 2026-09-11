import type { ReviewChange, ReviewResult } from './types';

export type ReviewResultBucket = 'aiRated' | 'aiRejected' | 'pending';

/**
 * 按有效评分划分三个互斥结果区，人工锁定属于来源属性。
 * 已明确保存的人工评分同样有效，不能继续计入待复核队列。
 */
export function reviewResultBucket(result: ReviewResult): ReviewResultBucket {
  if (reviewResultNeedsAttention(result)) return 'pending';
  if (result.rating === 0) return 'aiRejected';
  return result.rating >= 1 && result.rating <= 5 ? 'aiRated' : 'pending';
}

export function reviewResultIsWritable(result: ReviewResult) {
  if (result.source === 'manual') return result.rating >= 0 && result.rating <= 5;
  return !result.requiresHumanReview && result.rating >= 0 && result.rating <= 5;
}

export function reviewResultNeedsAttention(result: ReviewResult) {
  return result.source === 'ai' && result.requiresHumanReview;
}

/** A color-only edit must not confirm the placeholder rating of an unresolved result. */
export function reviewChangeFor(
  result: ReviewResult,
  patch: Partial<Pick<ReviewResult, 'rating' | 'colorLabel'>>,
): ReviewChange | null {
  if (reviewResultNeedsAttention(result) && patch.rating === undefined) return null;
  return {
    resultId: result.resultId,
    rating: patch.rating ?? result.rating,
    colorLabel: patch.colorLabel === undefined ? result.colorLabel : patch.colorLabel,
  };
}
