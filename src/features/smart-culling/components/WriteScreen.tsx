import { CheckCircle2, FileWarning, RefreshCcw, ShieldCheck } from 'lucide-react';
import { useProcessStore } from '../../../store/useProcessStore';
import { useSmartCullingFailureText } from '../errorText';
import { useSmartCullingText } from '../i18n';
import type { SmartCullingSnapshot } from '../types';
import { runSmartCullingCommand, useSmartCullingStore } from '../useSmartCulling';
import { LifecycleChrome, fileName } from './LifecycleChrome';
import { SmartCullingImage } from './SmartCullingImage';

export function WriteScreen({
  snapshot,
  onExit,
  onRefresh,
}: {
  snapshot: SmartCullingSnapshot;
  onExit: () => void;
  onRefresh?: () => void | Promise<void>;
}) {
  const tx = useSmartCullingText();
  const failureText = useSmartCullingFailureText();
  const thumbnails = useProcessStore((state) => state.thumbnails);
  const { busy, setState } = useSmartCullingStore();
  const summary = snapshot.writeSummary;
  const writeFailures = snapshot.failures.filter((failure) => failure.stage === 'write');
  const retry = async () => {
    try {
      await runSmartCullingCommand({ action: 'retryFailures' });
      await onRefresh?.();
    } catch {
      // Preserve the completed task and its failure list for another retry.
    }
  };
  const exit = async () => {
    try {
      await onRefresh?.();
      await runSmartCullingCommand({ action: 'abandon' });
      onExit();
    } catch {
      // Do not hide a completed task until its in-memory state is released.
    }
  };
  return (
    <div className="sc-page">
      <LifecycleChrome screen="write">
        <span className="sc-status good">
          <CheckCircle2 size={14} />
          {tx('applied')}
        </span>
      </LifecycleChrome>
      <main className="sc-write-shell">
        <header>
          <span>
            <CheckCircle2 size={28} />
          </span>
          <div>
            <h1>{tx('applied')}</h1>
            <p>
              {summary?.succeeded ?? 0} {tx('writtenSummary')} · {summary?.failed ?? 0} {tx('failedSummary')}
            </p>
          </div>
        </header>
        <section className="sc-write-metrics">
          <article>
            <strong>{summary?.succeeded ?? 0}</strong>
            <span>{tx('writeSuccess')}</span>
          </article>
          <article>
            <strong>{summary?.failed ?? 0}</strong>
            <span>{tx('writeFailed')}</span>
          </article>
          <article>
            <strong>{summary?.protected ?? 0}</strong>
            <span>{tx('existingProtected')}</span>
          </article>
          <article>
            <strong>{summary?.skipped ?? 0}</strong>
            <span>{tx('writeSkipped')}</span>
          </article>
        </section>
        <div className="sc-write-columns">
          <section>
            <header>
              <h2>{tx('recentSuccess')}</h2>
              <span>{tx('sidecarWritten')}</span>
            </header>
            {summary?.succeededPaths.slice(0, 12).map((path) => (
              <article key={path}>
                {thumbnails[path] ? (
                  <SmartCullingImage
                    primaryUrl={thumbnails[path]}
                    alt={fileName(path)}
                    fallback={<span className="sc-file-placeholder" />}
                  />
                ) : (
                  <span className="sc-file-placeholder" />
                )}
                <div>
                  <strong>{fileName(path)}</strong>
                  <small>{tx('metadataWritten')}</small>
                </div>
                <CheckCircle2 size={17} />
              </article>
            ))}
          </section>
          <section>
            <header>
              <h2>{tx('writeFailed')}</h2>
              <span>
                {writeFailures.length} {tx('itemsViewable')}
              </span>
            </header>
            {writeFailures.map((failure) => (
              <article key={`${failure.path}-${failure.code}`}>
                <FileWarning size={18} />
                <div>
                  <strong>{fileName(failure.path)}</strong>
                  <small>{failureText(failure)}</small>
                </div>
                {failure.retryable ? (
                  <button disabled={busy} onClick={() => void retry()}>
                    <RefreshCcw size={14} />
                    {tx('retry')}
                  </button>
                ) : null}
              </article>
            ))}
            <aside>
              <ShieldCheck size={16} />
              <span>{tx('successfulKept')}</span>
              {(summary?.skipped ?? 0) > 0 ? <small>{tx('writeSkippedHint')}</small> : null}
            </aside>
          </section>
        </div>
        <footer>
          <button className="sc-secondary" onClick={() => setState({ screen: 'review' })}>
            {tx('viewResults')}
          </button>
          <button className="sc-primary" onClick={() => void exit()}>
            {tx('returnLibrary')}
          </button>
        </footer>
      </main>
    </div>
  );
}
