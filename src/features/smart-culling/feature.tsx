import type { AppFeatureRegistration, LibraryFeatureFilterGroup } from '../contracts';
import SmartCullingEntry from './SmartCullingEntry';
import SmartCullingReviewPage from './SmartCullingReviewPage';
import SmartCullingThumbnailBadge from './SmartCullingThumbnailBadge';
import { SMART_CULLING_REVIEW_VIEW } from './constants';
import { getSmartCullingImageMetadata } from './metadata';

const SMART_CULLING_FILTER_GROUPS: LibraryFeatureFilterGroup[] = [
  {
    key: 'smartCulling',
    label: '智能选图',
    options: [
      {
        value: 'selected',
        label: '智能精选',
        predicate: ({ image }) => getSmartCullingImageMetadata(image)?.status === 'selected',
      },
      {
        value: 'review',
        label: '智能待确认',
        predicate: ({ image }) => getSmartCullingImageMetadata(image)?.status === 'review',
      },
      {
        value: 'reject_suggestion',
        label: '智能淘汰建议',
        predicate: ({ image }) => getSmartCullingImageMetadata(image)?.status === 'reject_suggestion',
      },
      {
        value: 'group_best',
        label: '相似组最优',
        predicate: ({ image }) => {
          const smart = getSmartCullingImageMetadata(image);
          return Boolean(smart?.groupId) && smart?.groupRank === 1;
        },
      },
      {
        value: 'group_folded',
        label: '相似组折叠项',
        predicate: ({ image }) => {
          const smart = getSmartCullingImageMetadata(image);
          return Boolean(smart?.groupId) && Boolean(smart?.groupRank && smart.groupRank > 1);
        },
      },
      {
        value: 'unprocessed',
        label: '未智能处理',
        predicate: ({ image }) => !getSmartCullingImageMetadata(image)?.status,
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
