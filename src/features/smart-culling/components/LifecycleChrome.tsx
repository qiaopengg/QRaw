import { Check, X } from 'lucide-react';
import { useSmartCullingText } from '../i18n';
import type { LifecycleScreen } from '../types';

const APP_NAME = 'QRaw';

function stageIndex(screen: LifecycleScreen) {
  if (screen === 'people' || screen === 'unsupported') return 0;
  if (screen === 'analysis' || screen === 'ready') return 1;
  if (screen === 'review') return 2;
  return 3;
}

export function LifecycleChrome({ screen, children }: { screen: LifecycleScreen; children?: React.ReactNode }) {
  const active = stageIndex(screen);
  const tx = useSmartCullingText();
  const stages = [tx('configureStage'), tx('analysisStage'), tx('reviewStage'), tx('confirmStage')];
  return (
    <header className="sc-topbar">
      <strong className="sc-brand">{APP_NAME}</strong>
      <nav className="sc-steps" aria-label="Smart culling progress">
        {stages.map((stage, index) => (
          <div key={stage} className={index === active ? 'is-active' : index < active ? 'is-done' : ''}>
            <span>{index < active ? <Check size={12} /> : index + 1}</span>
            <em>{stage}</em>
          </div>
        ))}
      </nav>
      <div className="sc-top-actions">{children}</div>
    </header>
  );
}

export function Modal({ children, onClose }: { children: React.ReactNode; onClose?: () => void }) {
  return (
    <div className="sc-modal-backdrop" role="dialog" aria-modal="true" onMouseDown={onClose}>
      <section className="sc-modal" onMouseDown={(event) => event.stopPropagation()}>
        {onClose ? (
          <button className="sc-modal-close" onClick={onClose} aria-label="Close">
            <X size={18} />
          </button>
        ) : null}
        {children}
      </section>
    </div>
  );
}

export function formatEta(seconds: number | null) {
  if (seconds === null) return '--:--';
  const minutes = Math.floor(seconds / 60);
  return `${String(minutes).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`;
}

export function fileName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}
