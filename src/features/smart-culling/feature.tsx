import type { AppFeatureRegistration } from '../contracts';
import SmartCullingEditorGuard from './SmartCullingEditorGuard';
import SmartCullingEntry from './SmartCullingEntry';
import SmartCullingLockControl from './SmartCullingLockControl';
import SmartCullingReviewPage from './SmartCullingReviewPage';
import SmartCullingThumbnailBadge from './SmartCullingThumbnailBadge';
import { SMART_CULLING_VIEW } from './constants';
import './smart-culling.css';
import './smart-culling-workbench.css';
import './smart-culling-review.css';
import './smart-culling-gallery.css';
import './styles/setup-decision-queue.css';
import './styles/analysis-decision-queue.css';
import './styles/review-decision-queue.css';
import './styles/accessibility.css';

export function useSmartCullingFeature(): AppFeatureRegistration {
  return {
    editor: {
      toolbarControls: [SmartCullingEditorGuard],
    },
    library: {
      headerActions: [SmartCullingLockControl, SmartCullingEntry],
      thumbnailBadges: [SmartCullingThumbnailBadge],
      views: {
        [SMART_CULLING_VIEW]: SmartCullingReviewPage,
      },
    },
  };
}
