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
/* 方案 A 主题层：必须最后加载，用于覆盖上方全部样式表的布局与视觉。
   仅作用于 UI，不参与任何业务逻辑。详见文件头注释。 */
import './styles/theme-conductor.css';

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
