import { CheckCircle2, X } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useSmartCullingText } from '../i18n';
import type { SmartCullingSnapshot } from '../types';
import { fileName } from './LifecycleChrome';

export function SmartCullingReadyNotice({
  snapshot,
  onOpenReview,
}: {
  snapshot: SmartCullingSnapshot | null;
  onOpenReview: () => void;
}) {
  const tx = useSmartCullingText();
  const [dismissedTaskId, setDismissedTaskId] = useState<string | null>(null);
  const [visibleTaskId, setVisibleTaskId] = useState<string | null>(null);
  const isReady = snapshot?.state === 'readyForReview' && Boolean(snapshot.taskId);

  useEffect(() => {
    if (!isReady || !snapshot?.taskId || snapshot.taskId === dismissedTaskId) return;
    setVisibleTaskId(snapshot.taskId);
  }, [dismissedTaskId, isReady, snapshot?.taskId]);

  if (!isReady || !snapshot?.taskId || visibleTaskId !== snapshot.taskId) return null;

  const dismiss = () => {
    setDismissedTaskId(snapshot.taskId);
    setVisibleTaskId(null);
  };
  const completed = snapshot.progress.completed.toLocaleString();
  const total = snapshot.progress.total.toLocaleString();

  return (
    <section className="sc-ready-card" role="status" aria-live="polite">
      <span>
        <CheckCircle2 size={21} />
      </span>
      <div>
        <h2>{snapshot.progress.partial ? tx('partialReadyForReview') : tx('readyForReview')}</h2>
        <p>
          {snapshot.rootPath ? `${fileName(snapshot.rootPath)} · ` : ''}
          {tx('completed')} {completed} / {total} · {tx('awaitingReview')}
        </p>
        <small>{tx('unconfirmedNotWritten')}</small>
      </div>
      <button className="sc-ready-dismiss" onClick={dismiss} aria-label={tx('dismiss')}>
        <X size={15} />
      </button>
      <button
        className="sc-primary"
        onClick={() => {
          dismiss();
          onOpenReview();
        }}
      >
        {tx('openReview')}
      </button>
    </section>
  );
}
