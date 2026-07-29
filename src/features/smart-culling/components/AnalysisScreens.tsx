import {
  AlertTriangle,
  CheckCircle2,
  Cpu,
  ExternalLink,
  FolderOpen,
  LoaderCircle,
  MonitorX,
  ShieldCheck,
  X,
} from 'lucide-react';
import { useState } from 'react';
import { useProcessStore } from '../../../store/useProcessStore';
import type { ImageFile } from '../../../components/ui/AppProperties';
import { useSmartCullingText } from '../i18n';
import type { SmartCullingSnapshot } from '../types';
import { runSmartCullingCommand, useSmartCullingStore } from '../useSmartCulling';
import { LifecycleChrome, Modal, fileName, formatEta } from './LifecycleChrome';

function PhotoGrid({ images }: { images: ImageFile[] }) {
  const thumbnails = useProcessStore((state) => state.thumbnails);
  return (
    <section className="sc-analysis-grid">
      {images.slice(0, 18).map((image) => (
        <article key={image.path}>
          {thumbnails[image.path] ? (
            <img src={thumbnails[image.path]} alt={fileName(image.path)} />
          ) : (
            <div>{fileName(image.path)}</div>
          )}
          <span>{fileName(image.path)}</span>
        </article>
      ))}
    </section>
  );
}

export function AnalysisScreen({
  snapshot,
  images,
  onBrowseLibrary,
}: {
  snapshot: SmartCullingSnapshot;
  images: ImageFile[];
  onBrowseLibrary: () => void;
}) {
  const tx = useSmartCullingText();
  const { cancelOpen, setState } = useSmartCullingStore();
  const progress = snapshot.progress;
  return (
    <div className="sc-page">
      <LifecycleChrome screen="analysis">
        <span className="sc-status">
          <LoaderCircle className="animate-spin" size={14} />
          {tx('analyzing')}
        </span>
        <button className="sc-text-button" onClick={onBrowseLibrary}>
          <FolderOpen size={14} />
          {tx('library')}
        </button>
      </LifecycleChrome>
      <main className="sc-analysis-shell">
        <header>
          <div>
            <h1>{fileName(snapshot.rootPath ?? '')}</h1>
            <p>
              {tx('readonly')} · {snapshot.inventory.folderCount} {tx('foldersUnit')}
            </p>
          </div>
          <span className="sc-status good">
            <ShieldCheck size={14} />
            {tx('readonly')}
          </span>
        </header>
        <PhotoGrid images={images} />
        <section className="sc-analysis-dock">
          <div className="sc-running-icon">
            <LoaderCircle className="animate-spin" size={20} />
          </div>
          <div>
            <strong>{tx('renderedState')}</strong>
            <small>{tx('analysisSignals')}</small>
          </div>
          <div className="sc-progress">
            <span>
              {tx('completed')} {progress.completed.toLocaleString()} / {progress.total.toLocaleString()}
            </span>
            <em>
              {tx('eta')} {formatEta(progress.etaSeconds)}
            </em>
            <i>
              <b style={{ width: `${progress.percent}%` }} />
            </i>
          </div>
          <button className="sc-secondary" onClick={() => setState({ cancelOpen: true })}>
            <X size={15} />
            {tx('cancelTask')}
          </button>
        </section>
      </main>
      {cancelOpen ? (
        <Modal onClose={() => setState({ cancelOpen: false })}>
          <span className="sc-dialog-icon warning">
            <AlertTriangle size={22} />
          </span>
          <h2>{tx('cancelTaskTitle')}</h2>
          <p>{tx('cancelTaskBody')}</p>
          <div className="sc-dialog-facts">
            <span>
              <CheckCircle2 size={14} />
              {tx('keepCompleted')}
            </span>
            <span>
              <FolderOpen size={14} />
              {tx('noOriginalChanges')}
            </span>
          </div>
          <footer>
            <button className="sc-secondary" onClick={() => setState({ cancelOpen: false })}>
              {tx('keepAnalyzing')}
            </button>
            <button
              className="sc-danger"
              onClick={() => {
                setState({ cancelOpen: false });
                void runSmartCullingCommand({ action: 'cancel' }).catch(() => undefined);
              }}
            >
              {tx('cancelAndReview')}
            </button>
          </footer>
        </Modal>
      ) : null}
    </div>
  );
}

export function UnsupportedScreen({ snapshot, onExit }: { snapshot: SmartCullingSnapshot; onExit: () => void }) {
  const tx = useSmartCullingText();
  const [guideOpen, setGuideOpen] = useState(false);
  const reason =
    {
      unsupported_platform: tx('unsupportedPlatform'),
      gpu_rendering_unavailable: tx('gpuRenderingUnavailable'),
      gpu_inference_unavailable: tx('gpuInferenceUnavailable'),
      bundled_models_missing: tx('bundledModelsMissing'),
      bundled_models_invalid: tx('bundledModelsInvalid'),
    }[snapshot.device.reason ?? ''] ?? tx('gpuInferenceUnavailable');
  const modelsReady = !['bundled_models_missing', 'bundled_models_invalid'].includes(snapshot.device.reason ?? '');
  return (
    <div className="sc-page">
      <LifecycleChrome screen="unsupported" />
      <main className="sc-unsupported">
        <section>
          <span className="sc-unsupported-icon">
            <MonitorX size={30} />
          </span>
          <em>
            {tx('title')} · {tx('deviceCheck')}
          </em>
          <h1>{tx('unsupported')}</h1>
          <p>{tx('unsupportedBody')}</p>
          <div className="sc-device-check">
            <article>
              <Cpu size={18} />
              <div>
                <strong>{tx('gpuCapability')}</strong>
                <span>{snapshot.device.provider || snapshot.device.platform}</span>
              </div>
              <AlertTriangle size={17} />
            </article>
            <article>
              <ShieldCheck size={18} />
              <div>
                <strong>{tx('bundledModels')}</strong>
                <span>{modelsReady ? tx('bundledModelsReady') : reason}</span>
              </div>
              {modelsReady ? <CheckCircle2 size={17} /> : <AlertTriangle size={17} />}
            </article>
            <article>
              <ShieldCheck size={18} />
              <div>
                <strong>{tx('noFallback')}</strong>
                <span>{tx('noFallbackHint')}</span>
              </div>
              <CheckCircle2 size={17} />
            </article>
          </div>
          <p className="sc-unsupported-reason">{reason}</p>
          <aside>
            <strong>{tx('otherUnaffected')}</strong>
            <span>{tx('otherCapabilities')}</span>
          </aside>
          <footer>
            <button className="sc-secondary" onClick={() => setGuideOpen(true)}>
              <ExternalLink size={14} />
              {tx('candidateGuide')}
            </button>
            <button className="sc-primary" onClick={onExit}>
              {tx('returnLibrary')}
            </button>
          </footer>
        </section>
      </main>
      {guideOpen ? (
        <Modal onClose={() => setGuideOpen(false)}>
          <h2>{tx('guideTitle')}</h2>
          <p>{tx('guideBody')}</p>
          <div className="sc-dialog-facts">
            <span>{tx('guideMac')}</span>
            <span>{tx('guideWindows')}</span>
          </div>
          <footer>
            <button className="sc-primary" onClick={() => setGuideOpen(false)}>
              {tx('close')}
            </button>
          </footer>
        </Modal>
      ) : null}
    </div>
  );
}
