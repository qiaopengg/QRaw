import type { ReviewResult } from './types';

export type ReviewResultBucket = 'aiRated' | 'aiRejected' | 'pending';

/**
 * 将全部结果严格划分为三个互斥集合。
 * 人工结果继续进入待人工复核，避免删除“人工结果”筛选后出现无归属照片。
 */
export function reviewResultBucket(result: ReviewResult): ReviewResultBucket {
  if (result.source !== 'ai' || result.requiresHumanReview) return 'pending';
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
