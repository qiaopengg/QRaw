import { AlertCircle, Check, FileCheck2, LockKeyhole, ShieldCheck } from 'lucide-react';
import { useSmartCullingText } from '../i18n';
import { reviewResultIsWritable, reviewResultNeedsAttention } from '../reviewPolicy';
import type { SmartCullingSnapshot } from '../types';
import { runSmartCullingCommand, useSmartCullingStore } from '../useSmartCulling';
import { Modal } from './LifecycleChrome';

export function ConfirmModal({ snapshot }: { snapshot: SmartCullingSnapshot }) {
  const tx = useSmartCullingText();
  const { busy, manualSyncPending, setState } = useSmartCullingStore();
  const writable = snapshot.results.filter(reviewResultIsWritable);
  const manual = writable.filter((result) => result.source === 'manual').length;
  const needsAttention = snapshot.results.filter(reviewResultNeedsAttention).length;
  const confirm = async () => {
    if (manualSyncPending) return;
    try {
      const next = await runSmartCullingCommand({ action: 'confirm' });
      setState({ confirmOpen: false, screen: next.state === 'idle' ? 'setup' : 'write' });
    } catch {
      // Keep the confirmation context open; the global banner explains the failure.
    }
  };
  return (
    <Modal onClose={() => setState({ confirmOpen: false })}>
      <div className="sc-confirm-heading">
        <span className="sc-dialog-icon">
          <FileCheck2 size={22} />
        </span>
        <div>
          <h2>{tx('confirmTitle')}</h2>
          <p>{tx('confirmBody')}</p>
        </div>
      </div>
      <div className="sc-confirm-metrics">
        <article>
          <strong>{writable.length}</strong>
          <span>{tx('applyCount')}</span>
        </article>
        <article>
          <strong>{manual}</strong>
          <span>{tx('manualCount')}</span>
        </article>
        <article>
          <strong>{needsAttention}</strong>
          <span>{tx('humanReviewNotWritten')}</span>
        </article>
        <article>
          <strong>{snapshot.inventory.protectedAssets}</strong>
          <span>{tx('existingProtected')}</span>
        </article>
      </div>
      <div className="sc-confirm-recheck">
        <AlertCircle size={16} />
        <div>
          <strong>{tx('confirmationCheckTitle')}</strong>
          <p>{tx('confirmationCheckBody')}</p>
        </div>
      </div>
      <div className="sc-confirm-list">
        <p>
          <Check size={15} />
          <span>
            <strong>{tx('writeRatings')}</strong>
            <small>{tx('writeRatingsHint')}</small>
          </span>
        </p>
        <p>
          <ShieldCheck size={15} />
          <span>
            <strong>{tx('baselineCheck')}</strong>
            <small>{tx('baselineHint')}</small>
          </span>
        </p>
        <p>
          <LockKeyhole size={15} />
          <span>
            <strong>{tx('rawWrite')}</strong>
            <small>{tx('rawWriteHint')}</small>
          </span>
        </p>
      </div>
      <div className="sc-warning">
        <AlertCircle size={15} />
        <p>{tx('partialWriteHint')}</p>
      </div>
      <footer>
        <button className="sc-secondary" onClick={() => setState({ confirmOpen: false })}>
          {tx('back')}
        </button>
        <button
          className="sc-primary"
          disabled={busy || manualSyncPending || writable.length === 0}
          onClick={() => void confirm()}
        >
          {tx('confirmAndWrite')} {writable.length}
        </button>
      </footer>
    </Modal>
  );
}
