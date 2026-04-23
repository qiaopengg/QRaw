import { useTranslation } from 'react-i18next';
import { ChatMessage } from '../types';

interface StyleTransferResultCardProps {
  onApplyPreview(message: ChatMessage): void;
  onDiscardPreview(message: ChatMessage): void;
  isLoading: boolean;
  message: ChatMessage;
  onShowPreview(message: ChatMessage): void;
  onShowSource(message: ChatMessage): void;
  onToggleCompare(message: ChatMessage): void;
}

export function StyleTransferResultCard({
  onApplyPreview,
  onDiscardPreview,
  isLoading: _isLoading,
  message,
  onShowPreview,
  onShowSource,
  onToggleCompare,
}: StyleTransferResultCardProps) {
  const { t } = useTranslation();
  const canCompareVariants = Boolean(
    (message.pureGenerationImagePath || message.sourceImagePath) &&
    (message.postProcessedImagePath || message.previewImagePath),
  );

  if (!message.previewImagePath && !message.outputImagePath) {
    return null;
  }

  return (
    <div className="w-full rounded-lg border border-surface bg-surface/40 px-2 py-1.5 space-y-1">
      <div className="flex items-center justify-between">
        <span className="text-[10px] text-text-secondary">{t('chat.styleTransferGenerated')}</span>
        <div className="flex items-center gap-3">
          {message.previewImagePath && message.sourceImagePath && (
            <>
              <button
                onClick={() => onShowSource(message)}
                className="text-[10px] text-blue-400 hover:text-blue-300 transition-colors"
              >
                {t('chat.styleTransferViewSource')}
              </button>
              <button
                onClick={() => onShowPreview(message)}
                className="text-[10px] text-blue-400 hover:text-blue-300 transition-colors"
              >
                {t('chat.openPreview')}
              </button>
              {canCompareVariants && (
                <button
                  onClick={() => onToggleCompare(message)}
                  className="text-[10px] text-blue-400 hover:text-blue-300 transition-colors"
                >
                  {t('chat.styleTransferCompareToggle')}
                </button>
              )}
              <button
                onClick={() => onDiscardPreview(message)}
                className="text-[10px] text-amber-300 hover:text-amber-200 transition-colors"
              >
                {t('chat.styleTransferDiscardPreview')}
              </button>
              <button
                onClick={() => onApplyPreview(message)}
                className="text-[10px] text-green-300 hover:text-green-200 transition-colors"
              >
                {t('chat.styleTransferApplyDerivative')}
              </button>
            </>
          )}
          {message.outputImagePath && (
            <button
              onClick={() => onApplyPreview(message)}
              className="text-[10px] text-blue-400 hover:text-blue-300 transition-colors"
            >
              {t('chat.styleTransferEnterExportWorkflow')}
            </button>
          )}
        </div>
      </div>
      {message.executionMeta && (
        <div className="text-[9px] text-text-secondary/70">
          {t('chat.styleTransferStageLabel')}:{' '}
          {message.executionMeta.stage === 'preview'
            ? t('chat.styleTransferStagePreview')
            : message.executionMeta.stage === 'export'
              ? t('chat.styleTransferStageExport')
              : t('chat.styleTransferStageAnalysis')}{' '}
          · {t('chat.styleTransferEtaLabel')}: {message.executionMeta.expectedWaitRange}
        </div>
      )}
      {(message.previewWorkflowState || message.qualityGuardPassed) && (
        <div className="text-[9px] text-text-secondary/70">
          {message.qualityGuardPassed ? `${t('chat.styleTransferQualityGuardPassed')} · ` : ''}
          {message.previewWorkflowState
            ? `${t('chat.styleTransferPreviewStateLabel')}: ${t(`chat.styleTransferPreviewState.${message.previewWorkflowState}`)}`
            : ''}
        </div>
      )}
      {canCompareVariants && (
        <div className="text-[9px] text-text-secondary/70">{t('chat.styleTransferCompareHint')}</div>
      )}
      <div className="text-[9px] text-text-secondary/70">{t('chat.aiDisclaimer')}</div>
    </div>
  );
}
