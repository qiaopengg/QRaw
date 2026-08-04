import type { ReviewResult } from './types';

export function reviewResultIsWritable(result: ReviewResult) {
  if (result.source === 'manual') return result.rating >= 0 && result.rating <= 5;
  return !result.requiresHumanReview && result.rating >= 1 && result.rating <= 5;
}

export function reviewResultNeedsAttention(result: ReviewResult) {
  return result.source === 'ai' && result.requiresHumanReview;
}
