import { Dispatch, SetStateAction, useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
import { AppSettings, Invokes } from '../../../../ui/AppProperties';
import { Adjustments } from '../../../../../utils/adjustments';
import {
  ChatAdjustResponse,
  ChatMessage,
  StreamChunkPayload,
  StyleTransferModelStatusResponse,
  StyleTransferPreset,
  StyleTransferStrategyMode,
} from '../types';
import {
  clampStyleTransferConfig,
  formatStyleTransferConfig,
  getSimpleAdjustments,
  getStyleTransferRequestLabel,
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
  const [styleTransferPreset, setStyleTransferPreset] = useState<StyleTransferPreset>(
    appSettings?.styleTransferPreset || 'artistic',
  );
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
  const [styleTransferStrategyMode, setStyleTransferStrategyMode] = useState<StyleTransferStrategyMode>(
    appSettings?.styleTransferStrategyMode || 'safe',
  );
  const [styleTransferModelStatus, setStyleTransferModelStatus] = useState<StyleTransferModelStatusResponse | null>(
    null,
  );
  const [isPreparingStyleTransferModels, setIsPreparingStyleTransferModels] = useState(false);
  const activeRunTokenRef = useRef<string | null>(null);

  const parseStyleTransferProgress = useCallback((rawText: string) => {
    const progressPattern = /\[PROGRESS\][^\n]*\((\d+)%\)\s*-\s*([^\n]+)/g;
    let lastMatch: RegExpExecArray | null = null;
    let nextMatch: RegExpExecArray | null = progressPattern.exec(rawText);
    while (nextMatch) {
      lastMatch = nextMatch;
      nextMatch = progressPattern.exec(rawText);
    }
    if (!lastMatch) return null;

    const percentage = parseInt(lastMatch[1], 10);
    if (!isFinite(percentage)) return null;

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
      const parsed = parseFloat(raw);
      const clamped = isFinite(parsed) ? clampStyleTransferConfig(parsed) : 1.0;
      persistAppSettings({ [key]: clamped } as Partial<AppSettings>);
      return formatStyleTransferConfig(clamped);
    },
    [persistAppSettings],
  );

  const getEffectiveStyleTransferTuning = useCallback((): StyleTransferTuningValues => {
    const parseOrDefault = (raw: string, fallback: number) => {
      const parsed = parseFloat(raw);
      if (isFinite(parsed)) {
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
    setStyleTransferPreset(appSettings?.styleTransferPreset || 'artistic');
  }, [appSettings?.styleTransferPreset]);

  useEffect(() => {
    setStyleTransferStrategyMode(appSettings?.styleTransferStrategyMode || 'safe');
  }, [appSettings?.styleTransferStrategyMode]);

  const refreshStyleTransferModelStatus = useCallback(() => {
    return invoke<StyleTransferModelStatusResponse>(Invokes.GetStyleTransferModelStatus)
      .then((status) => {
        setStyleTransferModelStatus(status);
        return status;
      })
      .catch((error) => {
        setError(String(error));
        return null;
      });
  }, [setError]);

  useEffect(() => {
    void refreshStyleTransferModelStatus();
  }, [refreshStyleTransferModelStatus]);

  const updateStyleTransferPreset = useCallback(
    (preset: StyleTransferPreset) => {
      setStyleTransferPreset(preset);
      persistAppSettings({ styleTransferPreset: preset });
    },
    [persistAppSettings],
  );

  const updateStyleTransferStrategyMode = useCallback(
    (mode: StyleTransferStrategyMode) => {
      setStyleTransferStrategyMode(mode);
      persistAppSettings({ styleTransferStrategyMode: mode });
    },
    [persistAppSettings],
  );

  const prepareStyleTransferModels = useCallback(() => {
    setIsPreparingStyleTransferModels(true);
    setError(null);
    return invoke<StyleTransferModelStatusResponse>(Invokes.PrepareStyleTransferModels)
      .then((status) => {
        setStyleTransferModelStatus(status);
        return status;
      })
      .catch((error) => {
        setError(String(error));
        throw error;
      })
      .finally(() => {
        setIsPreparingStyleTransferModels(false);
      });
  }, [setError]);

  const runStyleTransfer = useCallback(
    ({
      mainReferencePath,
      auxReferencePaths,
      sourceImagePath,
    }: {
      mainReferencePath: string;
      auxReferencePaths: string[];
      sourceImagePath: string;
    }) => {
      let streamMsgId: string | null = null;

      setError(null);
      setIsLoading(true);

      const userMsg: ChatMessage = {
        id: crypto.randomUUID(),
        role: 'user',
        content: getStyleTransferRequestLabel('analysis', t),
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
        referencePath: mainReferencePath,
        mainReferencePath,
        auxReferencePaths,
        sourceImagePath,
        requestedMode: 'analysis',
        strategyMode: styleTransferStrategyMode,
      };
      setMessages((prev) => [...prev, userMsg, streamMsg]);

      const simpleAdj = getSimpleAdjustments(adjustments);
      const tuning = getEffectiveStyleTransferTuning();
      const appendRunNote = (note: string) => {
        setMessages((prev) =>
          prev.map((msg) =>
            msg.id === currentStreamMsgId
              ? {
                  ...msg,
                  content:
                    msg.content.indexOf(note) >= 0 ? msg.content : `${msg.content}${msg.content ? '\n' : ''}${note}`,
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
        unlisten();
      };

      const slowWarningTimer = window.setTimeout(() => {
        if (activeRunTokenRef.current !== runToken) return;
        appendRunNote(t('chat.styleTransferSlowWarning'));
      }, 30_000);

      const finalizeRun = () => {
        if (activeRunTokenRef.current === runToken) {
          activeRunTokenRef.current = null;
          setIsLoading(false);
        }
        cleanupRun();
      };

      return listen<StreamChunkPayload>('style-transfer-stream', (event) => {
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
      })
        .then((stopListening) => {
          unlisten = stopListening;
          return invoke<ChatAdjustResponse>(Invokes.RunStyleTransfer, {
            request: {
              referencePath: mainReferencePath,
              mainReferencePath,
              auxReferencePaths,
              currentImagePath: sourceImagePath,
              currentAdjustments: simpleAdj,
              mode: 'analysis',
              strategyMode: styleTransferStrategyMode,
              preset: styleTransferPreset,
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
        })
        .then((result) => {
          if (activeRunTokenRef.current === runToken) {
            applyAssistantResult(currentStreamMsgId, result, { sourceImagePath });
          }
        })
        .catch((error) => {
          const errorText = String(error);
          if (activeRunTokenRef.current === runToken) {
            setError(errorText);
          }
          if (streamMsgId && activeRunTokenRef.current === runToken) {
            appendStreamError(streamMsgId, errorText);
          }
          finalizeRun();
        })
        .then(() => {
          finalizeRun();
        });
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
      styleTransferPreset,
      styleTransferStrategyMode,
      t,
    ],
  );

  const handleStyleTransfer = useCallback(() => {
    if (isLoading) return;
    if (!currentImagePath) {
      setError(t('chat.noImageOpen'));
      return;
    }

    return openFileDialog({
      multiple: true,
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
    })
      .then((selected) => {
        if (!selected) return;

        const selectedPaths = Array.isArray(selected) ? selected : [selected];
        const sanitizedPaths = selectedPaths.filter((path): path is string => typeof path === 'string' && !!path);
        if (!sanitizedPaths.length) return;
        const [mainReferencePath, ...auxReferencePaths] = sanitizedPaths;

        return runStyleTransfer({
          mainReferencePath,
          auxReferencePaths,
          sourceImagePath: currentImagePath,
        });
      })
      .catch((error) => {
        setError(String(error));
      });
  }, [currentImagePath, isLoading, runStyleTransfer, setError, t]);

  return {
    enableStyleTransferAutoRefine,
    enableStyleTransferExpertPreset,
    enableStyleTransferFeatureMapping,
    enableStyleTransferLut,
    enableStyleTransferVlm,
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
    skinProtectInput,
    isPreparingStyleTransferModels,
    styleStrengthInput,
    styleTransferModelStatus,
    styleTransferPreset,
    styleTransferStrategyMode,
    prepareStyleTransferModels,
    refreshStyleTransferModelStatus,
    updateStyleTransferStrategyMode,
    updateStyleTransferPreset,
  };
}
