import { Bot, LockKeyhole } from 'lucide-react';
import type { LibraryThumbnailBadgeSlotProps } from '../contracts';
import { useSmartCullingReasonText, useSmartCullingText } from './i18n';
import { getSmartCullingImageMetadata } from './metadata';

export default function SmartCullingThumbnailBadge({ image }: LibraryThumbnailBadgeSlotProps) {
  const tx = useSmartCullingText();
  const reasonText = useSmartCullingReasonText();
  const smart = getSmartCullingImageMetadata(image);
  if (!smart) return null;
  const reason = smart.source === 'ai' ? reasonText(smart.reasonCodes ?? []) : '';
  return (
    <div
      className={`sc-library-badge ${reason ? 'has-reason' : ''}`}
      data-tooltip={reason || (smart.source === 'ai' ? tx('ai') : tx('manual'))}
    >
      <span className="sc-library-badge-main">
        {smart.source === 'ai' ? <Bot size={11} /> : <LockKeyhole size={11} />}
        <span>{smart.source === 'ai' ? tx('ai') : tx('manualShort')}</span>
      </span>
      {reason ? <small>{reason}</small> : null}
    </div>
  );
}
