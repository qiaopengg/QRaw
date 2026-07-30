import { AlertTriangle, ArchiveX, CheckCircle2 } from 'lucide-react';
import { useEffect } from 'react';
import type { LibraryFeatureViewSlotProps } from '../contracts';
import { AnalysisScreen, UnsupportedScreen } from './components/AnalysisScreens';
import { Modal } from './components/LifecycleChrome';
import { PeopleScreen } from './components/PeopleScreen';
import { ReviewWorkbench } from './components/ReviewWorkbench';
import { SetupScreen } from './components/SetupScreen';
import { WriteScreen } from './components/WriteScreen';
import { useSmartCullingCommandErrorText } from './errorText';
import { useSmartCullingText } from './i18n';
import { runSmartCullingCommand, useSmartCullingStore } from './useSmartCulling';
import { useSmartCullingEvents } from './useSmartCullingEvents';

export default function SmartCullingReviewPage({
  currentFolderPath,
  imageList,
  allImageList,
  onBackToLibrary,
  onLibraryRefresh,
  onRequestThumbnails,
}: LibraryFeatureViewSlotProps) {
  useSmartCullingEvents();
  const tx = useSmartCullingText();
  const errorText = useSmartCullingCommandErrorText();
  const { snapshot, screen, abandonOpen, busy, error, setState } = useSmartCullingStore();
  const images = allImageList ?? imageList;
  useEffect(() => {
    if (!snapshot && currentFolderPath && !busy && !error) {
      void runSmartCullingCommand({ action: 'inspect', rootPath: currentFolderPath }).catch(() => undefined);
    }
  }, [busy, currentFolderPath, error, snapshot]);
  const abandon = async () => {
    try {
      await runSmartCullingCommand({ action: 'abandon' });
      setState({ abandonOpen: false });
      onBackToLibrary();
    } catch {
      // The feature-level error banner keeps the user in review with context.
    }
  };
  if (!snapshot)
    return (
      <div className="sc-page sc-loading">
        {tx('loading')}
        {error ? (
          <p className="sc-command-error" role="alert" title={error.detail}>
            {errorText(error)}
          </p>
        ) : null}
      </div>
    );
  let content;
  if (screen === 'unsupported') content = <UnsupportedScreen snapshot={snapshot} onExit={onBackToLibrary} />;
  else if (screen === 'people') content = <PeopleScreen snapshot={snapshot} images={images} />;
  else if (screen === 'analysis')
    content = <AnalysisScreen snapshot={snapshot} images={images} onBrowseLibrary={onBackToLibrary} />;
  else if (screen === 'review')
    content = <ReviewWorkbench snapshot={snapshot} onRequestThumbnails={onRequestThumbnails} />;
  else if (screen === 'write')
    content = <WriteScreen snapshot={snapshot} onExit={onBackToLibrary} onRefresh={onLibraryRefresh} />;
  else content = <SetupScreen snapshot={snapshot} onExit={() => setState({ abandonOpen: true })} />;
  const leavingReview = screen === 'review';
  return (
    <>
      {content}
      {error ? (
        <div className="sc-command-error" role="alert" title={error.detail}>
          {errorText(error)}
        </div>
      ) : null}
      {abandonOpen ? (
        <Modal onClose={() => setState({ abandonOpen: false })}>
          <span className="sc-dialog-icon warning">
            <AlertTriangle size={22} />
          </span>
          <h2>{tx(leavingReview ? 'abandonTitle' : 'cancelSetupTitle')}</h2>
          <p>{tx(leavingReview ? 'abandonBody' : 'cancelSetupBody')}</p>
          <div className="sc-dialog-facts">
            <span>
              <CheckCircle2 size={14} />
              {tx('originalsUnchanged')}
            </span>
            <span>
              <ArchiveX size={14} />
              {tx('temporaryCleared')}
            </span>
          </div>
          <footer>
            <button className="sc-secondary" onClick={() => setState({ abandonOpen: false })}>
              {tx(leavingReview ? 'continueReview' : 'continueSetup')}
            </button>
            <button className="sc-danger" onClick={() => void abandon()}>
              {tx('abandon')}
            </button>
          </footer>
        </Modal>
      ) : null}
    </>
  );
}
