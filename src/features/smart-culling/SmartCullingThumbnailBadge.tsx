import { Sparkles, Star } from 'lucide-react';
import type { LibraryThumbnailBadgeSlotProps } from '../contracts';
import Text from '../../components/ui/Text';
import { TextColors, TextVariants, TextWeights } from '../../types/typography';

export default function SmartCullingThumbnailBadge({ image }: LibraryThumbnailBadgeSlotProps) {
  const smart = image.featureData?.smartCulling;
  if (!smart || !smart.rating || smart.status === 'skipped') return null;

  return (
    <div
      className="inline-flex items-center gap-1 rounded-full bg-black/55 px-1.5 py-0.5 backdrop-blur-md shadow-md"
      data-tooltip={smart.reasonText || '智能选图建议'}
    >
      <Sparkles size={11} className="text-accent" />
      <Text variant={TextVariants.small} color={TextColors.white} weight={TextWeights.semibold}>
        AI {smart.rating}
      </Text>
      <Star size={10} className="text-white fill-white" />
    </div>
  );
}
