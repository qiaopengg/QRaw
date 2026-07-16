import { AlertCircle, Check, FileCheck2, LockKeyhole, ShieldCheck } from 'lucide-react';
import { useSmartCullingText } from '../i18n';
import type { SmartCullingSnapshot } from '../types';
import { runSmartCullingCommand, useSmartCullingStore } from '../useSmartCulling';
import { Modal } from './LifecycleChrome';

export function ConfirmModal({ snapshot }: { snapshot: SmartCullingSnapshot }) {
  const tx = useSmartCullingText();
  const { busy, setState } = useSmartCullingStore();
  const adopted = snapshot.results.filter((result) => result.adopted);
  const manual = adopted.filter((result) => result.source === 'manual').length;
  const confirm = async () => {
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
          <strong>{adopted.length}</strong>
          <span>{tx('applyCount')}</span>
        </article>
        <article>
          <strong>{manual}</strong>
          <span>{tx('manualCount')}</span>
        </article>
        <article>
          <strong>{snapshot.inventory.protectedAssets}</strong>
          <span>{tx('existingProtected')}</span>
        </article>
        <article>
          <strong>
            {snapshot.failures.filter((failure) => ['render', 'analysis'].includes(failure.stage)).length}
          </strong>
          <span>{tx('analysisFailures')}</span>
        </article>
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
        <button className="sc-primary" disabled={busy} onClick={() => void confirm()}>
          {tx('confirmAndWrite')} {adopted.length}
        </button>
      </footer>
    </Modal>
  );
}
