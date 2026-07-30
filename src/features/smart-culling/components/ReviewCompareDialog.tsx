import { useState } from 'react';
import { useSmartCullingText } from '../i18n';
import type { ReviewResult } from '../types';
import { Modal } from './LifecycleChrome';
import { ReviewCompareWorkspace } from './ReviewCompareWorkspace';

export function ReviewCompareDialog({
  first,
  second,
  onClose,
  onFocus,
  onToggle,
  onRating,
  onLabel,
  onOpenInspector,
  readOnly,
}: {
  first: ReviewResult;
  second: ReviewResult;
  onClose: () => void;
  onFocus: (resultId: string) => void;
  onToggle: (result: ReviewResult) => void;
  onRating: (result: ReviewResult, rating: number) => void;
  onLabel: (result: ReviewResult, colorLabel: ReviewResult['colorLabel']) => void;
  onOpenInspector: () => void;
  readOnly: boolean;
}) {
  const tx = useSmartCullingText();
  const [primaryResultId, setPrimaryResultId] = useState(first.resultId);
  return (
    <Modal onClose={onClose}>
      <div className="sc-compare-dialog-title">{tx('compareSelected')}</div>
      <ReviewCompareWorkspace
        results={[first, second]}
        focusedResultId={primaryResultId}
        onFocus={(resultId) => {
          setPrimaryResultId(resultId);
          onFocus(resultId);
        }}
        onToggle={onToggle}
        onRating={onRating}
        onLabel={onLabel}
        onOpenInspector={onOpenInspector}
        readOnly={readOnly}
      />
    </Modal>
  );
}
