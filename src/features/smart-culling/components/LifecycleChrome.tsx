import { Check, X } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { useSmartCullingText } from '../i18n';
import type { LifecycleScreen } from '../types';

const APP_NAME = 'QRaw';

function stageIndex(screen: LifecycleScreen) {
  if (screen === 'setup' || screen === 'unsupported') return 0;
  if (screen === 'analysis') return 1;
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
  const tx = useSmartCullingText();
  const dialogRef = useRef<HTMLElement>(null);

  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const firstControl = dialogRef.current?.querySelector<HTMLElement>(
      'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    );
    (firstControl ?? dialogRef.current)?.focus();
    return () => previousFocus?.focus();
  }, []);

  return (
    <div className="sc-modal-backdrop" onMouseDown={onClose}>
      <section
        ref={dialogRef}
        className="sc-modal"
        role="dialog"
        aria-modal="true"
        aria-label={tx('title')}
        tabIndex={-1}
        onMouseDown={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          if (event.key === 'Escape' && onClose) {
            event.stopPropagation();
            onClose();
            return;
          }
          if (event.key !== 'Tab') return;
          const controls = Array.from(
            event.currentTarget.querySelectorAll<HTMLElement>(
              'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
            ),
          );
          if (controls.length === 0) {
            event.preventDefault();
            event.currentTarget.focus();
            return;
          }
          const first = controls[0];
          const last = controls[controls.length - 1];
          if (event.shiftKey && document.activeElement === first) {
            event.preventDefault();
            last.focus();
          } else if (!event.shiftKey && document.activeElement === last) {
            event.preventDefault();
            first.focus();
          }
        }}
      >
        {onClose ? (
          <button className="sc-modal-close" onClick={onClose} aria-label={tx('close')}>
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
