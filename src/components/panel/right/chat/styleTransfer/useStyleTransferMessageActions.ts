import { useCallback } from 'react';
import { Adjustments } from '../../../../../utils/adjustments';
import {
  AdjustmentValue,
  ChatAdjustResponse,
  ChatMessage,
  ChatOpenImageOptions,
  StyleConstraintAction,
} from '../types';
import { mergeAdjustments } from './utils';

type SliderChangeEvent = { target: { value: number | string } } | React.ChangeEvent<HTMLInputElement>;

interface UseStyleTransferMessageActionsParams {
  onOpenImage?(path: string, options?: ChatOpenImageOptions): void;
  setAdjustments(updater: (prev: Adjustments) => Adjustments): void;
  setMessages: React.Dispatch<React.SetStateAction<ChatMessage[]>>;
}

interface AssistantResultContext {
  sourceImagePath?: string | null;
}

function clampAdjustmentValue(key: string, value: number): number {
  if (key === 'exposure') return Math.max(-2.5, Math.min(2.5, Number(value.toFixed(2))));
  if (key === 'sharpness') return Math.max(0, Math.min(100, Math.round(value)));
  return Math.max(-80, Math.min(80, Math.round(value)));
}

export function useStyleTransferMessageActions({
  onOpenImage,
  setAdjustments,
  setMessages,
}: UseStyleTransferMessageActionsParams) {
  const patchPreviewWorkflowState = useCallback(
    (messageId: string, previewWorkflowState: ChatMessage['previewWorkflowState']) => {
      setMessages((prev) => prev.map((msg) => (msg.id === messageId ? { ...msg, previewWorkflowState } : msg)));
    },
    [setMessages],
  );

  const handleSliderChange = useCallback(
    (msgId: string, key: string, event: SliderChangeEvent) => {
      const numericValue = parseFloat(String(event.target.value));
      if (isNaN(numericValue)) return;

      setMessages((prev) =>
        prev.map((msg) => {
          if (msg.id !== msgId || !msg.adjustments) return msg;
          return { ...msg, appliedValues: { ...(msg.appliedValues || {}), [key]: numericValue } };
        }),
      );
      setAdjustments((prev) => ({ ...prev, [key]: numericValue }));
    },
    [setAdjustments, setMessages],
  );

  const handleHslSliderChange = useCallback(
    (msgId: string, color: string, channel: 'hue' | 'saturation' | 'luminance', event: SliderChangeEvent) => {
      const numericValue = parseFloat(String(event.target.value));
      if (isNaN(numericValue)) return;

      setMessages((prev) =>
        prev.map((msg) => {
          if (msg.id !== msgId || !msg.adjustments) return msg;
          const keyPath = `hsl.${color}.${channel}`;
          return { ...msg, appliedValues: { ...(msg.appliedValues || {}), [keyPath]: numericValue } };
        }),
      );

      setAdjustments((prev) =>
        mergeAdjustments(prev, {
          hsl: {
            [color]: {
              [channel]: numericValue,
            },
          },
        } as unknown as Partial<Adjustments>),
      );
    },
    [setAdjustments, setMessages],
  );

  const applyAllSuggestions = useCallback(
    (message: ChatMessage) => {
      if (!message.adjustments) return;

      const updates: Partial<Adjustments> = {};
      message.adjustments.forEach((suggestion) => {
        (updates as Record<string, AdjustmentValue>)[suggestion.key] =
          suggestion.complex_value !== undefined ? suggestion.complex_value : suggestion.value;
      });

      setAdjustments((prev) => mergeAdjustments(prev, updates));
    },
    [setAdjustments],
  );

  const applyConstraintActions = useCallback(
    (msgId: string, actions: StyleConstraintAction[]) => {
      if (!actions.length) return;

      const patch: Record<string, number> = {};
      setAdjustments((prev) => {
        const next = { ...prev };
        actions.forEach((action) => {
          const current = Number(next[action.key as keyof Adjustments] ?? 0);
          const updated = clampAdjustmentValue(action.key, current + action.delta);
          (next as Record<string, number>)[action.key] = updated;
          patch[action.key] = updated;
        });
        return next;
      });

      setMessages((prev) =>
        prev.map((msg) =>
          msg.id === msgId ? { ...msg, appliedValues: { ...(msg.appliedValues || {}), ...patch } } : msg,
        ),
      );
    },
    [setAdjustments, setMessages],
  );

  const showStyleTransferPreview = useCallback(
    (message: ChatMessage) => {
      if (!message.previewImagePath || !message.sourceImagePath) return;
      onOpenImage?.(message.previewImagePath, {
        styleTransferSession: {
          mode: 'styleTransferPreview',
          sourcePath: message.sourceImagePath,
          previewPath: message.previewImagePath,
        },
      });
      patchPreviewWorkflowState(message.id, 'preview');
    },
    [onOpenImage, patchPreviewWorkflowState],
  );

  const showStyleTransferSource = useCallback(
    (message: ChatMessage) => {
      if (!message.previewImagePath || !message.sourceImagePath) return;
      onOpenImage?.(message.sourceImagePath, {
        preserveAdjustments: true,
        styleTransferSession: {
          mode: 'styleTransferSource',
          sourcePath: message.sourceImagePath,
          previewPath: message.previewImagePath,
        },
      });
      patchPreviewWorkflowState(message.id, 'source');
    },
    [onOpenImage, patchPreviewWorkflowState],
  );

  const toggleStyleTransferCompare = useCallback(
    (message: ChatMessage) => {
      const compareBasePathCandidate = message.pureGenerationImagePath || message.sourceImagePath;
      const compareTargetPathCandidate = message.postProcessedImagePath || message.previewImagePath;
      if (!compareBasePathCandidate || !compareTargetPathCandidate) return;
      const compareBasePath: string = compareBasePathCandidate;
      const compareTargetPath: string = compareTargetPathCandidate;
      const sourcePath: string = message.sourceImagePath || compareBasePath;
      onOpenImage?.(compareBasePath, {
        preserveAdjustments: true,
        styleTransferSession: {
          mode: 'styleTransferCompare',
          sourcePath,
          previewPath: compareTargetPath,
          compareBasePath,
          compareTargetPath,
        },
      });
      patchPreviewWorkflowState(message.id, 'compare');
    },
    [onOpenImage, patchPreviewWorkflowState],
  );

  const discardStyleTransferPreview = useCallback(
    (message: ChatMessage) => {
      if (!message.previewImagePath || !message.sourceImagePath) return;
      onOpenImage?.(message.sourceImagePath, {
        preserveAdjustments: true,
        styleTransferSession: {
          mode: 'styleTransferDiscard',
          sourcePath: message.sourceImagePath,
          previewPath: message.previewImagePath,
        },
      });
      patchPreviewWorkflowState(message.id, 'discarded');
    },
    [onOpenImage, patchPreviewWorkflowState],
  );

  const applyStyleTransferPreview = useCallback(
    (message: ChatMessage) => {
      if (message.previewImagePath && message.sourceImagePath) {
        onOpenImage?.(message.previewImagePath, {
          styleTransferSession: {
            mode: 'styleTransferApply',
            sourcePath: message.sourceImagePath,
            previewPath: message.previewImagePath,
          },
        });
        patchPreviewWorkflowState(message.id, 'applied');
        return;
      }

      if (message.outputImagePath) {
        onOpenImage?.(message.outputImagePath, {
          activatePanel: 'export',
          preserveAdjustments: false,
        });
        patchPreviewWorkflowState(message.id, 'exported');
      }
    },
    [onOpenImage, patchPreviewWorkflowState],
  );

  const applyAssistantResult = useCallback(
    (msgId: string, result: ChatAdjustResponse, context?: AssistantResultContext) => {
      const updates: Partial<Adjustments> = {};
      result.adjustments.forEach((suggestion) => {
        (updates as Record<string, AdjustmentValue>)[suggestion.key] =
          suggestion.complex_value !== undefined ? suggestion.complex_value : suggestion.value;
      });

      if (Object.keys(updates).length > 0) {
        setAdjustments((prev) => mergeAdjustments(prev, updates));
      }

      if (result.previewImagePath && result.executionMeta?.resolvedMode === 'generativePreview') {
        onOpenImage?.(result.previewImagePath, {
          styleTransferSession: {
            mode: 'styleTransferPreview',
            sourcePath: context?.sourceImagePath || '',
            previewPath: result.previewImagePath,
          },
        });
      }

      if (result.outputImagePath && result.executionMeta?.resolvedMode === 'generativeExport') {
        onOpenImage?.(result.outputImagePath, {
          activatePanel: 'export',
          preserveAdjustments: false,
        });
      }

      setMessages((prev) =>
        prev.map((msg) =>
          msg.id === msgId
            ? {
                ...msg,
                content: result.understanding,
                adjustments: result.adjustments,
                appliedValues: Object.fromEntries(
                  result.adjustments.map((suggestion) => [
                    suggestion.key,
                    suggestion.complex_value !== undefined ? suggestion.complex_value : suggestion.value,
                  ]),
                ),
                styleDebug: result.style_debug,
                constraintDebug: result.constraint_debug ?? result.style_debug?.constraint_debug,
                executionMeta: result.executionMeta,
                serviceStatus: result.serviceStatus,
                outputImagePath: result.outputImagePath,
                previewImagePath: result.previewImagePath,
                pureGenerationImagePath: result.pureGenerationImagePath,
                postProcessedImagePath: result.postProcessedImagePath,
                styleTransferProgress: undefined,
                previewWorkflowState:
                  result.executionMeta?.resolvedMode === 'generativePreview'
                    ? 'preview'
                    : result.executionMeta?.resolvedMode === 'generativeExport'
                      ? 'exported'
                      : msg.previewWorkflowState,
                qualityGuardPassed: Boolean(result.previewImagePath || result.outputImagePath),
              }
            : msg,
        ),
      );
    },
    [onOpenImage, setAdjustments, setMessages],
  );

  return {
    applyAllSuggestions,
    applyAssistantResult,
    applyConstraintActions,
    applyStyleTransferPreview,
    discardStyleTransferPreview,
    handleHslSliderChange,
    handleSliderChange,
    showStyleTransferPreview,
    showStyleTransferSource,
    toggleStyleTransferCompare,
  };
}
