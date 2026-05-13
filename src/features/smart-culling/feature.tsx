import type { AppFeatureRegistration, LibraryFeatureFilterGroup } from '../contracts';
import SmartCullingEntry from './SmartCullingEntry';
import SmartCullingReviewPage from './SmartCullingReviewPage';
import SmartCullingThumbnailBadge from './SmartCullingThumbnailBadge';
import { SMART_CULLING_REVIEW_VIEW } from './constants';

const SMART_CULLING_FILTER_GROUPS: LibraryFeatureFilterGroup[] = [
  {
    key: 'smartCulling',
    label: '智能选图',
    options: [
      {
        value: 'selected',
        label: '智能精选',
        predicate: ({ image }) => image.featureData?.smartCulling?.status === 'selected',
      },
      {
        value: 'review',
        label: '智能待确认',
        predicate: ({ image }) => image.featureData?.smartCulling?.status === 'review',
      },
      {
        value: 'reject_suggestion',
        label: '智能淘汰建议',
        predicate: ({ image }) => image.featureData?.smartCulling?.status === 'reject_suggestion',
      },
      {
        value: 'group_best',
        label: '相似组最优',
        predicate: ({ image }) =>
          Boolean(image.featureData?.smartCulling?.groupId) && image.featureData?.smartCulling?.groupRank === 1,
      },
      {
        value: 'group_folded',
        label: '相似组折叠项',
        predicate: ({ image }) =>
          Boolean(image.featureData?.smartCulling?.groupId) &&
          Boolean(image.featureData?.smartCulling?.groupRank && image.featureData.smartCulling.groupRank > 1),
      },
      {
        value: 'unprocessed',
        label: '未智能处理',
        predicate: ({ image }) => !image.featureData?.smartCulling?.status,
      },
    ],
  },
];

export function useSmartCullingFeature(): AppFeatureRegistration {
  return {
    library: {
      filterGroups: SMART_CULLING_FILTER_GROUPS,
      headerActions: [SmartCullingEntry],
      thumbnailBadges: [SmartCullingThumbnailBadge],
      views: {
        [SMART_CULLING_REVIEW_VIEW]: SmartCullingReviewPage,
      },
    },
  };
}
