import { useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Invokes } from '../../../../ui/AppProperties';
import { Adjustments, INITIAL_MASK_ADJUSTMENTS } from '../../../../../utils/adjustments';
import { SubMaskMode } from '../../Masks';
import {
  AdjustmentValue,
  AppliedValueMap,
  ChatAdjustResponse,
  ChatMessage,
  ChatOpenImageOptions,
  StyleConstraintAction,
} from '../types';
import { mergeAdjustments } from './utils';

type SliderChangeEvent = { target: { value: number | string } } | React.ChangeEvent<HTMLInputElement>;

const STYLE_TRANSFER_ADJUSTMENT_KEYS = new Set<keyof Adjustments>([
  'exposure',
  'brightness',
  'contrast',
  'highlights',
  'shadows',
  'whites',
  'blacks',
  'saturation',
  'vibrance',
  'temperature',
  'tint',
  'clarity',
  'dehaze',
  'structure',
  'sharpness',
  'grainAmount',
  'grainSize',
  'grainRoughness',
  'glowAmount',
  'halationAmount',
  'flareAmount',
  'vignetteAmount',
  'vignetteFeather',
  'vignetteMidpoint',
  'vignetteRoundness',
  'lutData',
  'lutIntensity',
  'lutName',
  'lutPath',
  'lutSize',
]);

interface UseStyleTransferMessageActionsParams {
  adjustments: Adjustments;
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
  adjustments,
  onOpenImage,
  setAdjustments,
  setMessages,
}: UseStyleTransferMessageActionsParams) {
  const buildAppliedValueMap = useCallback((result: ChatAdjustResponse) => {
    const applied: AppliedValueMap = {};
    result.adjustments.forEach((suggestion) => {
      applied[suggestion.key] =
        suggestion.complex_value !== undefined ? suggestion.complex_value : suggestion.value;
    });

    const globalPatch = result.guardedGlobalAdjustments ?? result.globalAdjustments;
    if (globalPatch && typeof globalPatch === 'object') {
      for (const [key, value] of Object.entries(globalPatch)) {
        applied[key] = value;
      }
    }
    if (result.curves) {
      applied.curves = result.curves;
    }
    if (result.hsl) {
      applied.hsl = result.hsl;
    }
    if (result.globalLut && typeof result.globalLut === 'object') {
      for (const [key, value] of Object.entries(result.globalLut)) {
        applied[key] = value;
      }
    }
    return applied;
  }, []);

  const buildMasksFromLocalRegions = useCallback((message: ChatMessage, prevMasks: Adjustments['masks']) => {
    const regions = message.guardedLocalRegions?.length ? message.guardedLocalRegions : message.localRegions;
    if (!regions?.length) return prevMasks;

    const existingIds = new Set((prevMasks || []).map((mask) => mask.id));
    const nextMasks = [...(prevMasks || [])];
    regions.forEach((region) => {
      const maskId = `style-transfer-${region.id}`;
      if (existingIds.has(maskId)) return;
      nextMasks.push({
        id: maskId,
        name: region.label,
        visible: !region.defaultHidden,
        invert: false,
        opacity: 100,
        adjustments: {
          ...INITIAL_MASK_ADJUSTMENTS,
          ...(region.adjustments as Record<string, unknown>),
        },
        subMasks: [
          {
            id: `${maskId}-submask`,
            type: region.maskHint.maskType,
            visible: !region.defaultHidden,
            invert: false,
            opacity: 100,
            mode: SubMaskMode.Additive,
            parameters: region.maskHint.parameters,
          },
        ],
      } as unknown as Adjustments['masks'][number]);
    });
    return nextMasks;
  }, []);

  const buildStructuredPatch = useCallback(
    (result: ChatAdjustResponse, prev: Adjustments): Partial<Adjustments> => {
      const updates: Partial<Adjustments> = {};
      const assignKnownAdjustmentValues = (source?: Record<string, unknown> | null) => {
        if (!source || typeof source !== 'object') return;
        for (const [key, value] of Object.entries(source)) {
          if (STYLE_TRANSFER_ADJUSTMENT_KEYS.has(key as keyof Adjustments)) {
            (updates as Record<string, AdjustmentValue>)[key] = value;
          }
        }
      };

      assignKnownAdjustmentValues(result.guardedGlobalAdjustments ?? result.globalAdjustments);

      result.sliderMapping?.forEach((entry) => {
        if (entry.target !== 'adjustments') return;
        if (!STYLE_TRANSFER_ADJUSTMENT_KEYS.has(entry.key as keyof Adjustments)) return;
        (updates as Record<string, AdjustmentValue>)[entry.key] =
          entry.complexValue !== undefined ? entry.complexValue : entry.value;
      });

      if (!Object.keys(updates).length) {
        result.adjustments.forEach((suggestion) => {
          if (!STYLE_TRANSFER_ADJUSTMENT_KEYS.has(suggestion.key as keyof Adjustments)) return;
          (updates as Record<string, AdjustmentValue>)[suggestion.key] =
            suggestion.complex_value !== undefined ? suggestion.complex_value : suggestion.value;
        });
      }
      if (result.curves) {
        (updates as Record<string, unknown>).curves = result.curves;
      }
      if (result.hsl) {
        (updates as Record<string, unknown>).hsl = result.hsl;
      }
      if (result.globalLut) {
        Object.assign(updates as Record<string, unknown>, result.globalLut);
      }

      const nextMasks = buildMasksFromLocalRegions(
        {
          id: '',
          role: 'assistant',
          content: '',
          localRegions: result.localRegions,
          guardedLocalRegions: result.guardedLocalRegions,
        } as ChatMessage,
        prev.masks,
      );
      if (nextMasks !== prev.masks) {
        updates.masks = nextMasks;
      }

      return updates;
    },
    [buildMasksFromLocalRegions],
  );

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

      setAdjustments((prev) =>
        mergeAdjustments(
          prev,
          buildStructuredPatch(
            {
              understanding: message.content,
              adjustments: message.adjustments ?? [],
              globalAdjustments: message.globalAdjustments,
              curves: message.curves,
              hsl: message.hsl,
              globalLut: message.globalLut,
              localRegions: message.localRegions,
              guardedLocalRegions: message.guardedLocalRegions,
            },
            prev,
          ),
        ),
      );
    },
    [buildStructuredPatch, setAdjustments],
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
        void invoke(Invokes.SaveStyleTransferSidecar, {
          path: message.sourceImagePath,
          adjustments,
          styleTransfer: {
            inputSignature: message.sourceImagePath,
            mainReferenceSignature: message.mainReferencePath || message.referencePath || null,
            auxReferenceSignatures: message.auxReferencePaths || [],
            mode: message.requestedMode || 'analysis',
            modelVersion: message.executionMeta?.engine || null,
            globalAdjustments: message.globalAdjustments || {},
            curves: message.curves || null,
            hsl: message.hsl || null,
            globalLut: message.globalLut || null,
            localRegions: message.guardedLocalRegions || message.localRegions || [],
            sliderMapping: message.sliderMapping || [],
            riskWarnings: message.riskWarnings || [],
          },
        }).catch(() => {});
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
      setAdjustments((prev) => mergeAdjustments(prev, buildStructuredPatch(result, prev)));

      const previewPath = result.previewImagePath || result.outputImagePath;

      if (previewPath) {
        onOpenImage?.(previewPath, {
          styleTransferSession: {
            mode: 'styleTransferPreview',
            sourcePath: context?.sourceImagePath || '',
            previewPath,
          },
        });
      }

      setMessages((prev) =>
        prev.map((msg) =>
          msg.id === msgId
            ? {
                ...msg,
                content: result.understanding,
                adjustments: result.adjustments,
                styleDebug: result.style_debug,
                constraintDebug: result.constraint_debug ?? result.style_debug?.constraint_debug,
                globalAdjustments: result.globalAdjustments,
                guardedGlobalAdjustments: result.guardedGlobalAdjustments,
                curves: result.curves,
                hsl: result.hsl,
                globalLut: result.globalLut,
                localRegions: result.localRegions,
                guardedLocalRegions: result.guardedLocalRegions,
                qualityReport: result.qualityReport,
                riskWarnings: result.riskWarnings,
                sliderMapping: result.sliderMapping,
                executionMeta: result.executionMeta,
                processingDebug: result.processingDebug,
                modelStatus: result.modelStatus,
                outputImagePath: result.outputImagePath,
                previewImagePath: result.previewImagePath,
                pureGenerationImagePath: result.pureGenerationImagePath,
                postProcessedImagePath: result.postProcessedImagePath,
                styleTransferProgress: undefined,
                previewWorkflowState: previewPath ? 'preview' : msg.previewWorkflowState,
                qualityGuardPassed: Boolean(result.previewImagePath || result.outputImagePath),
                appliedValues: buildAppliedValueMap(result),
              }
            : msg,
        ),
      );
    },
    [buildAppliedValueMap, buildStructuredPatch, onOpenImage, setAdjustments, setMessages],
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
