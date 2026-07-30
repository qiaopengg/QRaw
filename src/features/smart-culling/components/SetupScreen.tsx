import { Check, ChevronRight, FolderTree, Info, ShieldCheck, UserRoundCheck, Zap } from 'lucide-react';
import { SMART_CULLING_MODES } from '../constants';
import { useSmartCullingModes, useSmartCullingText } from '../i18n';
import type { SmartCullingSnapshot } from '../types';
import { runSmartCullingCommand, useSmartCullingStore } from '../useSmartCulling';
import { LifecycleChrome, fileName } from './LifecycleChrome';

export function SetupScreen({ snapshot, onExit }: { snapshot: SmartCullingSnapshot; onExit: () => void }) {
  const tx = useSmartCullingText();
  const modeCopy = useSmartCullingModes();
  const { mode, keyPeople, busy, setState } = useSmartCullingStore();
  const start = () =>
    snapshot.rootPath &&
    runSmartCullingCommand({ action: 'start', rootPath: snapshot.rootPath, mode, keyPeople }).catch(() => undefined);
  return (
    <div className="sc-page">
      <LifecycleChrome screen="setup">
        <span className="sc-status good">
          <ShieldCheck size={14} />
          {tx('deviceReady')}
        </span>
      </LifecycleChrome>
      <main className="sc-setup-shell">
        <aside className="sc-context-sidebar">
          <strong>{fileName(snapshot.rootPath ?? '')}</strong>
          <span>
            {snapshot.inventory.folderCount} {tx('foldersUnit')}
          </span>
          <p>
            {snapshot.device.provider}
            <br />
            {snapshot.device.modelVersion}
          </p>
        </aside>
        <section className="sc-setup-workspace">
          <header className="sc-heading">
            <span>{tx('title')}</span>
            <h1>{tx('setupTitle')}</h1>
            <p>{tx('setupDescription')}</p>
          </header>
          <div className="sc-setup-grid">
            <section className="sc-setup-card">
              <div className="sc-section-title">
                <b>1</b>
                <div>
                  <h2>{tx('chooseMode')}</h2>
                  <p>{tx('chooseModeHint')}</p>
                </div>
              </div>
              <div className="sc-mode-grid">
                {SMART_CULLING_MODES.map((id) => (
                  <button key={id} className={mode === id ? 'is-selected' : ''} onClick={() => setState({ mode: id })}>
                    <i>{mode === id ? <Check size={12} /> : null}</i>
                    <strong>{modeCopy[id][0]}</strong>
                    <small>{modeCopy[id][1]}</small>
                  </button>
                ))}
              </div>
              <div className="sc-section-title sc-people-title">
                <b>2</b>
                <div>
                  <h2>
                    {tx('keyPeople')} <em>{tx('optional')}</em>
                  </h2>
                  <p>{tx('keyPeopleHint')}</p>
                </div>
              </div>
              <button className="sc-people-entry" onClick={() => setState({ screen: 'people' })}>
                <UserRoundCheck size={20} />
                <span>
                  <strong>{tx('choosePeople')}</strong>
                  <small>{keyPeople.length ? `${keyPeople.length} ${tx('selectedPeople')}` : tx('noPeople')}</small>
                </span>
                <ChevronRight size={18} />
              </button>
            </section>
            <aside className="sc-summary-card">
              <h2>{tx('taskSummary')}</h2>
              <dl>
                <div>
                  <dt>
                    <FolderTree size={15} />
                    {tx('scope')}
                  </dt>
                  <dd>
                    {fileName(snapshot.rootPath ?? '')} · {snapshot.inventory.folderCount} {tx('foldersUnit')}
                  </dd>
                </div>
                <div>
                  <dt>
                    <Zap size={15} />
                    {tx('estimated')}
                  </dt>
                  <dd>
                    {snapshot.inventory.totalAssets.toLocaleString()} {tx('assetsUnit')}
                  </dd>
                </div>
                <div>
                  <dt>
                    <ShieldCheck size={15} />
                    {tx('manualProtection')}
                  </dt>
                  <dd>
                    {snapshot.inventory.protectedAssets.toLocaleString()} {tx('protectedSuffix')}
                  </dd>
                </div>
              </dl>
              <div className="sc-info">
                <Info size={15} />
                <p>{tx('formatNote')}</p>
              </div>
              <div className="sc-actions">
                <button className="sc-secondary" onClick={onExit}>
                  {tx('cancel')}
                </button>
                <button
                  className="sc-primary"
                  disabled={busy || snapshot.inventory.eligibleAssets === 0}
                  onClick={() => void start()}
                >
                  {tx('start')}
                </button>
              </div>
              <small>{tx('offline')}</small>
            </aside>
          </div>
        </section>
      </main>
    </div>
  );
}
