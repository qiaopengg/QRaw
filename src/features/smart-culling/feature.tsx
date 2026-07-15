import type { AppFeatureRegistration } from '../contracts';
import SmartCullingEntry from './SmartCullingEntry';
import SmartCullingReviewPage from './SmartCullingReviewPage';
import SmartCullingThumbnailBadge from './SmartCullingThumbnailBadge';
import { SMART_CULLING_REVIEW_VIEW } from './constants';

export function useSmartCullingFeature(): AppFeatureRegistration {
  return {
    library: {
      headerActions: [SmartCullingEntry],
      thumbnailBadges: [SmartCullingThumbnailBadge],
      views: {
        [SMART_CULLING_REVIEW_VIEW]: SmartCullingReviewPage,
      },
    },
  };
}
