import {
  AlertTriangle,
  Check,
  ChevronDown,
  ChevronUp,
  CopySlash,
  FolderOpen,
  ShieldCheck,
  UserRoundCheck,
} from 'lucide-react';
import { useState } from 'react';
import type { ImageFile } from '../../../components/ui/AppProperties';
import { SMART_CULLING_MODES, smartCullingModeSupportsKeyPeople } from '../constants';
import { useSmartCullingModes, useSmartCullingText } from '../i18n';
import type { SmartCullingSnapshot } from '../types';
import { runSmartCullingCommand, useSmartCullingStore } from '../useSmartCulling';
import { LifecycleChrome } from './LifecycleChrome';
import { KeyPeoplePicker } from './KeyPeoplePicker';

export function SetupScreen({
  snapshot,
  images,
  ignoredVirtualCopies = 0,
  onExit,
  onRequestThumbnails,
}: {
  snapshot: SmartCullingSnapshot;
  images: ImageFile[];
  ignoredVirtualCopies?: number;
  onExit: () => void;
  onRequestThumbnails?: (paths: string[]) => void;
}) {
  const tx = useSmartCullingText();
  const modeCopy = useSmartCullingModes();
  const { mode, keyPeople, busy, setState } = useSmartCullingStore();
  const [peopleOpen, setPeopleOpen] = useState(false);
  const interruptedTask = snapshot.failures.find((failure) => failure.code === 'previous_task_interrupted');
  const supportsKeyPeople = smartCullingModeSupportsKeyPeople(mode);
  const selectMode = (nextMode: typeof mode) => {
    const nextSupportsKeyPeople = smartCullingModeSupportsKeyPeople(nextMode);
    if (!nextSupportsKeyPeople) setPeopleOpen(false);
    setState({
      mode: nextMode,
      keyPeople: nextSupportsKeyPeople ? keyPeople : [],
    });
  };
  const start = () =>
    snapshot.rootPath &&
    runSmartCullingCommand({
      action: 'start',
      rootPath: snapshot.rootPath,
      mode,
      keyPeople: supportsKeyPeople ? keyPeople : [],
    }).catch(() => undefined);
  return (
    <div className="sc-page sc-setup-page">
      <LifecycleChrome screen="setup">
        <span className="sc-status good">
          <ShieldCheck size={14} />
          {tx('deviceReady')}
        </span>
      </LifecycleChrome>
      <main className="sc-setup-shell">
        <section className="sc-setup-workspace">
          <header className="sc-heading">
            <span>{tx('title')}</span>
            <h1>{tx('setupTitle')}</h1>
            <p>{tx('setupDescription')}</p>
            {snapshot.rootPath ? (
              <small className="sc-task-root" title={snapshot.rootPath}>
                <FolderOpen size={14} />
                {tx('taskFolder')}: {snapshot.rootPath}
              </small>
            ) : null}
          </header>
          {interruptedTask ? (
            <div className="sc-recovery-notice" role="status">
              <AlertTriangle size={18} />
              <div>
                <strong>{tx('previousTaskInterrupted')}</strong>
                <p>{tx('previousTaskInterruptedHint')}</p>
                <small>{interruptedTask.path}</small>
              </div>
            </div>
          ) : null}
          {ignoredVirtualCopies > 0 ? (
            <div className="sc-virtual-copy-notice" role="status">
              <CopySlash size={17} />
              <span>
                {ignoredVirtualCopies} {tx('virtualCopiesExcluded')}
              </span>
            </div>
          ) : null}
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
                  <button key={id} className={mode === id ? 'is-selected' : ''} onClick={() => selectMode(id)}>
                    <i>{mode === id ? <Check size={12} /> : null}</i>
                    <strong>{modeCopy[id][0]}</strong>
                    <small>{modeCopy[id][1]}</small>
                  </button>
                ))}
              </div>
              {supportsKeyPeople ? (
                <>
                  <div className="sc-section-title sc-people-title">
                    <b>2</b>
                    <div>
                      <h2>
                        {tx('keyPeopleSetupTitle')} <em>{tx('optional')}</em>
                      </h2>
                      <p>{tx('keyPeopleSetupHint')}</p>
                    </div>
                  </div>
                  <button
                    className={`sc-people-entry ${peopleOpen ? 'is-open' : ''}`}
                    aria-expanded={peopleOpen}
                    onClick={() => setPeopleOpen((open) => !open)}
                  >
                    <UserRoundCheck size={20} />
                    <span>
                      <strong>{tx('choosePeople')}</strong>
                      <small>{keyPeople.length ? `${keyPeople.length} ${tx('selectedPeople')}` : tx('noPeople')}</small>
                    </span>
                    {peopleOpen ? <ChevronUp size={18} /> : <ChevronDown size={18} />}
                  </button>
                  {peopleOpen ? (
                    <KeyPeoplePicker
                      snapshot={snapshot}
                      images={images}
                      onClose={() => setPeopleOpen(false)}
                      onRequestThumbnails={onRequestThumbnails}
                    />
                  ) : null}
                </>
              ) : null}
              <footer className="sc-setup-footer">
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
              </footer>
            </section>
          </div>
        </section>
      </main>
    </div>
  );
}
