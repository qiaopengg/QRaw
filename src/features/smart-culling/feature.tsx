import type { AppFeatureRegistration } from '../contracts';
import SmartCullingEditorGuard from './SmartCullingEditorGuard';
import SmartCullingEntry from './SmartCullingEntry';
import SmartCullingReviewPage from './SmartCullingReviewPage';
import SmartCullingThumbnailBadge from './SmartCullingThumbnailBadge';
import { SMART_CULLING_VIEW } from './constants';
import './smart-culling.css';
import './smart-culling-workbench.css';
import './smart-culling-review.css';

export function useSmartCullingFeature(): AppFeatureRegistration {
  return {
    editor: {
      toolbarControls: [SmartCullingEditorGuard],
    },
    library: {
      headerActions: [SmartCullingEntry],
      thumbnailBadges: [SmartCullingThumbnailBadge],
      views: {
        [SMART_CULLING_VIEW]: SmartCullingReviewPage,
      },
    },
  };
}
