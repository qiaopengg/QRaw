import { Dispatch, SetStateAction, useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { confirm as confirmDialog, open as openFileDialog } from '@tauri-apps/plugin-dialog';
import { AppSettings, Invokes } from '../../../../ui/AppProperties';
import { Adjustments } from '../../../../../utils/adjustments';
import {
  ChatAdjustResponse,
  ChatMessage,
  DEFAULT_STYLE_TRANSFER_SERVICE_URL,
  StreamChunkPayload,
  StyleTransferExportFormat,
  StyleTransferModeSetting,
  StyleTransferPreset,
  StyleTransferRequestMode,
  StyleTransferServiceStatus,
} from '../types';
import {
  clampStyleTransferConfig,
  formatStyleTransferConfig,
  getSimpleAdjustments,
  getStyleTransferRequestLabel,
  normalizeServiceUrl,
} from './utils';

interface StyleTransferTuningValues {
  styleStrength: number;
  highlightGuardStrength: number;
  skinProtectStrength: number;
}

interface UseStyleTransferParams {
  activeModel: string;
  adjustments: Adjustments;
  appSettings?: AppSettings | null;
  currentImagePath?: string | null;
  endpoint: string;
  isLoading: boolean;
  llmApiKey?: string;
  persistAppSettings(patch: Partial<AppSettings>): void;
  setError: Dispatch<SetStateAction<string | null>>;
  setIsLoading: Dispatch<SetStateAction<boolean>>;
  setMessages: Dispatch<SetStateAction<ChatMessage[]>>;
  applyAssistantResult(msgId: string, result: ChatAdjustResponse, context?: { sourceImagePath?: string | null }): void;
  t(key: string): string;
}

export function useStyleTransfer({
  activeModel,
  adjustments,
  appSettings,
  currentImagePath,
  endpoint,
  isLoading,
  llmApiKey,
  persistAppSettings,
  setError,
  setIsLoading,
  setMessages,
  applyAssistantResult,
  t,
}: UseStyleTransferParams) {
  const [styleTransferMode, setStyleTransferMode] = useState<StyleTransferModeSetting>(
    appSettings?.styleTransferMode || 'analysis',
  );
  const [styleTransferPreset, setStyleTransferPreset] = useState<StyleTransferPreset>(
    appSettings?.styleTransferPreset || 'artistic',
  );
  const [styleTransferExportFormat, setStyleTransferExportFormat] = useState<StyleTransferExportFormat>(
    appSettings?.styleTransferExportFormat || 'tiff',
  );
  const [styleTransferServiceUrl, setStyleTransferServiceUrl] = useState(
    appSettings?.styleTransferServiceUrl || DEFAULT_STYLE_TRANSFER_SERVICE_URL,
  );
  const [styleTransferEnableRefiner, setStyleTransferEnableRefiner] = useState(
    appSettings?.styleTransferEnableRefiner ?? false,
  );
  const [styleTransferAllowFallback, setStyleTransferAllowFallback] = useState(
    appSettings?.styleTransferAllowFallback ?? false,
  );
  const [styleTransferServiceStatus, setStyleTransferServiceStatus] = useState<StyleTransferServiceStatus | null>(null);
  const [checkingStyleTransferService, setCheckingStyleTransferService] = useState(false);
  const [styleStrengthInput, setStyleStrengthInput] = useState(
    formatStyleTransferConfig(appSettings?.styleTransferStrength ?? 1.0),
  );
  const [highlightGuardInput, setHighlightGuardInput] = useState(
    formatStyleTransferConfig(appSettings?.styleTransferHighlightGuard ?? 1.0),
  );
  const [skinProtectInput, setSkinProtectInput] = useState(
    formatStyleTransferConfig(appSettings?.styleTransferSkinProtect ?? 1.0),
  );
  const [pureStyleTransfer, setPureStyleTransfer] = useState(true);
  const [enableStyleTransferLut, setEnableStyleTransferLut] = useState(true);
  const [enableStyleTransferExpertPreset, setEnableStyleTransferExpertPreset] = useState(true);
  const [enableStyleTransferFeatureMapping, setEnableStyleTransferFeatureMapping] = useState(true);
  const [enableStyleTransferAutoRefine, setEnableStyleTransferAutoRefine] = useState(true);
  const [enableStyleTransferVlm, setEnableStyleTransferVlm] = useState(true);
  const activeRunTokenRef = useRef<string | null>(null);

  const parseStyleTransferProgress = useCallback((rawText: string) => {
    const matches = Array.from(rawText.matchAll(/\[PROGRESS\][^\n]*\((\d+)%\)\s*-\s*([^\n]+)/g));
    const lastMatch = matches.at(-1);
    if (!lastMatch) return null;

    const percentage = Number.parseInt(lastMatch[1], 10);
    if (!Number.isFinite(percentage)) return null;

    return {
      percentage: Math.max(0, Math.min(100, percentage)),
      description: lastMatch[2].trim(),
      rawText: lastMatch[0].trim(),
    };
  }, []);

  const appendStreamError = useCallback(
    (messageId: string, errorText: string) => {
      setMessages((prev) =>
        prev.map((msg) =>
          msg.id === messageId
            ? {
                ...msg,
                thinkingContent: `${msg.thinkingContent || ''}${msg.thinkingContent ? '\n' : ''}${errorText}`,
                styleTransferProgress: undefined,
              }
            : msg,
        ),
      );
    },
    [setMessages],
  );

  const saveStyleTransferConfig = useCallback(
    (key: 'styleTransferStrength' | 'styleTransferHighlightGuard' | 'styleTransferSkinProtect', raw: string) => {
      const parsed = Number.parseFloat(raw);
      const clamped = Number.isFinite(parsed) ? clampStyleTransferConfig(parsed) : 1.0;
      persistAppSettings({ [key]: clamped } as Partial<AppSettings>);
      return formatStyleTransferConfig(clamped);
    },
    [persistAppSettings],
  );

  const getEffectiveStyleTransferTuning = useCallback((): StyleTransferTuningValues => {
    const parseOrDefault = (raw: string, fallback: number) => {
      const parsed = Number.parseFloat(raw);
      if (Number.isFinite(parsed)) {
        return clampStyleTransferConfig(parsed);
      }
      return clampStyleTransferConfig(fallback);
    };

    return {
      styleStrength: parseOrDefault(styleStrengthInput, appSettings?.styleTransferStrength ?? 1.0),
      highlightGuardStrength: parseOrDefault(highlightGuardInput, appSettings?.styleTransferHighlightGuard ?? 1.0),
      skinProtectStrength: parseOrDefault(skinProtectInput, appSettings?.styleTransferSkinProtect ?? 1.0),
    };
  }, [
    appSettings?.styleTransferHighlightGuard,
    appSettings?.styleTransferSkinProtect,
    appSettings?.styleTransferStrength,
    highlightGuardInput,
    skinProtectInput,
    styleStrengthInput,
  ]);

  useEffect(() => {
    setStyleStrengthInput(formatStyleTransferConfig(appSettings?.styleTransferStrength ?? 1.0));
  }, [appSettings?.styleTransferStrength]);

  useEffect(() => {
    setHighlightGuardInput(formatStyleTransferConfig(appSettings?.styleTransferHighlightGuard ?? 1.0));
  }, [appSettings?.styleTransferHighlightGuard]);

  useEffect(() => {
    setSkinProtectInput(formatStyleTransferConfig(appSettings?.styleTransferSkinProtect ?? 1.0));
  }, [appSettings?.styleTransferSkinProtect]);

  useEffect(() => {
    setStyleTransferMode(appSettings?.styleTransferMode || 'analysis');
  }, [appSettings?.styleTransferMode]);

  useEffect(() => {
    setStyleTransferPreset(appSettings?.styleTransferPreset || 'artistic');
  }, [appSettings?.styleTransferPreset]);

  useEffect(() => {
    setStyleTransferExportFormat(appSettings?.styleTransferExportFormat || 'tiff');
  }, [appSettings?.styleTransferExportFormat]);

  useEffect(() => {
    setStyleTransferServiceUrl(appSettings?.styleTransferServiceUrl || DEFAULT_STYLE_TRANSFER_SERVICE_URL);
  }, [appSettings?.styleTransferServiceUrl]);

  useEffect(() => {
    setStyleTransferEnableRefiner(appSettings?.styleTransferEnableRefiner ?? false);
  }, [appSettings?.styleTransferEnableRefiner]);

  useEffect(() => {
    setStyleTransferAllowFallback(appSettings?.styleTransferAllowFallback ?? false);
  }, [appSettings?.styleTransferAllowFallback]);

  const checkStyleTransferService = useCallback(
    async (rawUrl?: string) => {
      const serviceUrl = normalizeServiceUrl(rawUrl ?? styleTransferServiceUrl);
      setCheckingStyleTransferService(true);
      try {
        const status = await invoke<StyleTransferServiceStatus>(Invokes.CheckStyleTransferService, {
          serviceUrl,
        });
        setStyleTransferServiceStatus(status);
        return status;
      } catch (error) {
        const status: StyleTransferServiceStatus = {
          serviceUrl,
          reachable: false,
          ready: false,
          status: 'error',
          capabilities: [],
          detail: String(error),
        };
        setStyleTransferServiceStatus(status);
        return status;
      } finally {
        setCheckingStyleTransferService(false);
      }
    },
    [styleTransferServiceUrl],
  );

  useEffect(() => {
    if (styleTransferMode === 'generativePreview') {
      void checkStyleTransferService();
    }
  }, [checkStyleTransferService, styleTransferMode]);

  const updateStyleTransferMode = useCallback(
    (mode: StyleTransferModeSetting) => {
      setStyleTransferMode(mode);
      persistAppSettings({ styleTransferMode: mode });
      if (mode === 'generativePreview') {
        void checkStyleTransferService();
      }
    },
    [checkStyleTransferService, persistAppSettings],
  );

  const updateStyleTransferPreset = useCallback(
    (preset: StyleTransferPreset) => {
      setStyleTransferPreset(preset);
      persistAppSettings({ styleTransferPreset: preset });
    },
    [persistAppSettings],
  );

  const updateStyleTransferExportFormat = useCallback(
    (format: StyleTransferExportFormat) => {
      setStyleTransferExportFormat(format);
      persistAppSettings({ styleTransferExportFormat: format });
    },
    [persistAppSettings],
  );

  const commitStyleTransferServiceUrl = useCallback(() => {
    const normalized = normalizeServiceUrl(styleTransferServiceUrl);
    setStyleTransferServiceUrl(normalized);
    persistAppSettings({ styleTransferServiceUrl: normalized });
    void checkStyleTransferService(normalized);
  }, [checkStyleTransferService, persistAppSettings, styleTransferServiceUrl]);

  const updateStyleTransferEnableRefiner = useCallback(
    (enabled: boolean) => {
      setStyleTransferEnableRefiner(enabled);
      persistAppSettings({ styleTransferEnableRefiner: enabled });
    },
    [persistAppSettings],
  );

  const updateStyleTransferAllowFallback = useCallback(
    (enabled: boolean) => {
      setStyleTransferAllowFallback(enabled);
      persistAppSettings({ styleTransferAllowFallback: enabled });
    },
    [persistAppSettings],
  );

  const runStyleTransfer = useCallback(
    async ({
      mode,
      referencePath,
      sourceImagePath,
    }: {
      mode: StyleTransferRequestMode;
      referencePath: string;
      sourceImagePath: string;
    }) => {
      let streamMsgId: string | null = null;

      setError(null);
      setIsLoading(true);

      const userMsg: ChatMessage = {
        id: crypto.randomUUID(),
        role: 'user',
        content: getStyleTransferRequestLabel(mode, t),
      };

      streamMsgId = crypto.randomUUID();
      const currentStreamMsgId = streamMsgId;
      const runToken = crypto.randomUUID();
      activeRunTokenRef.current = runToken;
      const streamMsg: ChatMessage = {
        id: currentStreamMsgId,
        role: 'assistant',
        content: '',
        thinkingContent: '',
        referencePath,
        sourceImagePath,
        requestedMode: mode,
      };
      setMessages((prev) => [...prev, userMsg, streamMsg]);

      const simpleAdj = getSimpleAdjustments(adjustments);
      const tuning = getEffectiveStyleTransferTuning();
      const isGenerativeRun = mode === 'generativePreview' || mode === 'generativeExport';
      const appendRunNote = (note: string) => {
        setMessages((prev) =>
          prev.map((msg) =>
            msg.id === currentStreamMsgId
              ? {
                  ...msg,
                  content: msg.content.includes(note) ? msg.content : `${msg.content}${msg.content ? '\n' : ''}${note}`,
                }
              : msg,
          ),
        );
      };

      let cleanedUp = false;
      let unlisten = () => {};
      const cleanupRun = () => {
        if (cleanedUp) return;
        cleanedUp = true;
        window.clearTimeout(slowWarningTimer);
        window.clearTimeout(timeoutConfirmTimer);
        unlisten();
      };

      const slowWarningTimer = window.setTimeout(() => {
        if (activeRunTokenRef.current !== runToken || !isGenerativeRun) return;
        appendRunNote(t('chat.styleTransferSlowWarning'));
      }, 30_000);

      const timeoutConfirmTimer = window.setTimeout(async () => {
        if (activeRunTokenRef.current !== runToken || !isGenerativeRun) return;
        appendRunNote(t('chat.styleTransferTimeoutConfirmPending'));

        let shouldContinue = false;
        try {
          shouldContinue = await confirmDialog(
            mode === 'generativeExport'
              ? t('chat.styleTransferRuntimeExportTimeoutPrompt')
              : t('chat.styleTransferRuntimePreviewTimeoutPrompt'),
            {
              title: t('chat.styleTransferTimeoutConfirmTitle'),
              kind: 'warning',
            },
          );
        } catch (dialogError) {
          console.error('Failed to open timeout confirm dialog:', dialogError);
          shouldContinue = window.confirm(
            mode === 'generativeExport'
              ? t('chat.styleTransferRuntimeExportTimeoutPrompt')
              : t('chat.styleTransferRuntimePreviewTimeoutPrompt'),
          );
        }

        if (activeRunTokenRef.current !== runToken) return;
        if (shouldContinue) {
          appendRunNote(t('chat.styleTransferContinueConfirmed'));
          return;
        }

        activeRunTokenRef.current = null;
        void invoke(Invokes.CancelStyleTransfer).catch((cancelError) => {
          console.error('Failed to cancel style transfer task:', cancelError);
        });
        cleanupRun();
        setIsLoading(false);
        setError(t('chat.styleTransferRuntimeStopped'));
        appendRunNote(t('chat.styleTransferRuntimeStopped'));
      }, 120_000);

      unlisten = await listen<StreamChunkPayload>('style-transfer-stream', (event) => {
        const { chunk_type, text: chunkText, result } = event.payload;
        if (activeRunTokenRef.current !== runToken) return;

        if (chunk_type === 'thinking') {
          const parsedProgress = parseStyleTransferProgress(chunkText);
          setMessages((prev) =>
            prev.map((msg) =>
              msg.id === currentStreamMsgId
                ? {
                    ...msg,
                    thinkingContent: (msg.thinkingContent || '') + chunkText,
                    styleTransferProgress: parsedProgress ?? msg.styleTransferProgress,
                  }
                : msg,
            ),
          );
        } else if (chunk_type === 'content') {
          setMessages((prev) =>
            prev.map((msg) => (msg.id === currentStreamMsgId ? { ...msg, content: msg.content + chunkText } : msg)),
          );
        } else if (chunk_type === 'error') {
          appendStreamError(currentStreamMsgId, chunkText);
        } else if (chunk_type === 'done' && result) {
          applyAssistantResult(currentStreamMsgId, result, { sourceImagePath });
        }
      });

      try {
        const result = await invoke<ChatAdjustResponse>(Invokes.RunStyleTransfer, {
          request: {
            referencePath,
            currentImagePath: sourceImagePath,
            currentAdjustments: simpleAdj,
            mode,
            preset: styleTransferPreset,
            serviceUrl: normalizeServiceUrl(styleTransferServiceUrl),
            enableRefiner: styleTransferEnableRefiner,
            allowFallbackToAnalysis: styleTransferAllowFallback,
            outputFormat: styleTransferExportFormat,
            styleStrength: tuning.styleStrength,
            highlightGuardStrength: tuning.highlightGuardStrength,
            skinProtectStrength: tuning.skinProtectStrength,
            pureAlgorithm: pureStyleTransfer,
            enableExpertPreset: enableStyleTransferExpertPreset,
            enableFeatureMapping: enableStyleTransferFeatureMapping,
            enableAutoRefine: enableStyleTransferAutoRefine,
            enableLut: enableStyleTransferLut,
            enableVlm: enableStyleTransferVlm,
            llmEndpoint: endpoint,
            llmApiKey: llmApiKey || null,
            llmModel: activeModel || null,
          },
        });

        if (activeRunTokenRef.current === runToken) {
          applyAssistantResult(currentStreamMsgId, result, { sourceImagePath });
        }
      } catch (error) {
        const errorText = String(error);
        if (activeRunTokenRef.current === runToken) {
          setError(errorText);
        }
        if (streamMsgId && activeRunTokenRef.current === runToken) {
          appendStreamError(streamMsgId, errorText);
        }
      } finally {
        if (activeRunTokenRef.current === runToken) {
          activeRunTokenRef.current = null;
          setIsLoading(false);
        }
        cleanupRun();
      }
    },
    [
      activeModel,
      adjustments,
      appendStreamError,
      applyAssistantResult,
      enableStyleTransferAutoRefine,
      enableStyleTransferExpertPreset,
      enableStyleTransferFeatureMapping,
      enableStyleTransferLut,
      enableStyleTransferVlm,
      endpoint,
      getEffectiveStyleTransferTuning,
      llmApiKey,
      parseStyleTransferProgress,
      pureStyleTransfer,
      setError,
      setIsLoading,
      setMessages,
      styleTransferAllowFallback,
      styleTransferEnableRefiner,
      styleTransferExportFormat,
      styleTransferPreset,
      styleTransferServiceUrl,
      t,
    ],
  );

  const handleStyleTransfer = useCallback(
    async (modeOverride?: StyleTransferModeSetting) => {
      if (isLoading) return;
      if (!currentImagePath) {
        setError(t('chat.noImageOpen'));
        return;
      }

      try {
        const selected = await openFileDialog({
          multiple: false,
          filters: [
            {
              name: t('chat.imageFiles'),
              extensions: [
                'jpg',
                'jpeg',
                'png',
                'tiff',
                'tif',
                'webp',
                'bmp',
                'dng',
                'nef',
                'cr2',
                'cr3',
                'arw',
                'raf',
                'orf',
                'rw2',
              ],
            },
          ],
        });
        if (!selected) return;

        const referencePath = typeof selected === 'string' ? selected : selected;
        if (!referencePath) return;

        await runStyleTransfer({
          mode: modeOverride || styleTransferMode,
          referencePath,
          sourceImagePath: currentImagePath,
        });
      } catch (error) {
        setError(String(error));
      }
    },
    [currentImagePath, isLoading, runStyleTransfer, setError, styleTransferMode, t],
  );

  const handleGenerativeExport = useCallback(
    async (msg: ChatMessage) => {
      if (isLoading || !msg.referencePath || !msg.sourceImagePath) return;
      let confirmed = false;
      try {
        confirmed = await confirmDialog(t('chat.styleTransferExportConfirmPrompt'), {
          title: t('chat.styleTransferTimeoutConfirmTitle'),
          kind: 'warning',
        });
      } catch (dialogError) {
        console.error('Failed to open export confirm dialog:', dialogError);
        confirmed = window.confirm(t('chat.styleTransferExportConfirmPrompt'));
      }
      if (!confirmed) return;
      await runStyleTransfer({
        mode: 'generativeExport',
        referencePath: msg.referencePath,
        sourceImagePath: msg.sourceImagePath,
      });
    },
    [isLoading, runStyleTransfer, t],
  );

  return {
    checkingStyleTransferService,
    checkStyleTransferService,
    commitStyleTransferServiceUrl,
    enableStyleTransferAutoRefine,
    enableStyleTransferExpertPreset,
    enableStyleTransferFeatureMapping,
    enableStyleTransferLut,
    enableStyleTransferVlm,
    handleGenerativeExport,
    handleStyleTransfer,
    highlightGuardInput,
    pureStyleTransfer,
    saveStyleTransferConfig,
    setEnableStyleTransferAutoRefine,
    setEnableStyleTransferExpertPreset,
    setEnableStyleTransferFeatureMapping,
    setEnableStyleTransferLut,
    setEnableStyleTransferVlm,
    setHighlightGuardInput,
    setPureStyleTransfer,
    setSkinProtectInput,
    setStyleStrengthInput,
    setStyleTransferServiceUrl,
    skinProtectInput,
    styleStrengthInput,
    styleTransferAllowFallback,
    styleTransferEnableRefiner,
    styleTransferExportFormat,
    styleTransferMode,
    styleTransferPreset,
    styleTransferServiceStatus,
    styleTransferServiceUrl,
    updateStyleTransferAllowFallback,
    updateStyleTransferEnableRefiner,
    updateStyleTransferExportFormat,
    updateStyleTransferMode,
    updateStyleTransferPreset,
  };
}
