import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { v4 as uuidv4 } from 'uuid';
import { motion, AnimatePresence } from 'framer-motion';
import {
  DndContext,
  DragOverlay,
  PointerSensor,
  useDraggable,
  useDroppable,
  useSensor,
  useSensors,
  DragEndEvent,
  DragStartEvent,
  pointerWithin,
} from '@dnd-kit/core';
import {
  Circle,
  Eye,
  EyeOff,
  FileEdit,
  Loader2,
  Minus,
  Plus,
  RotateCcw,
  Trash2,
  Wand2,
  Send,
  FolderOpen,
} from 'lucide-react';

import CollapsibleSection from '../../ui/CollapsibleSection';
import Switch from '../../ui/Switch';
import Slider from '../../ui/Slider';
import Input from '../../ui/Input';
import Button from '../../ui/Button';

import { useContextMenu } from '../../../context/ContextMenuContext';
import {
  Mask,
  MaskType,
  SubMask,
  SubMaskMode,
  ToolType,
  MASK_ICON_MAP,
  getAiPanelCreationTypes,
  getAiSubMaskComponentTypes,
} from './Masks';
import { Adjustments, AiPatch } from '../../../utils/adjustments';
import { BrushSettings, SelectedImage } from '../../ui/AppProperties';
import { createSubMask } from '../../../utils/maskUtils';

interface AiPanelProps {
  adjustments: Adjustments;
  activePatchContainerId: string | null;
  activeSubMaskId: string | null;
  aiModelDownloadStatus: string | null;
  brushSettings: BrushSettings | null;
  isAIConnectorConnected: boolean;
  isGeneratingAi: boolean;
  isGeneratingAiMask: boolean;
  onDeletePatch(id: string): void;
  onGenerateAiForegroundMask(id: string): void;
  onGenerativeReplace(patchId: string, prompt: any, useFastInpaint: boolean): void;
  onSelectPatchContainer(id: string | null): void;
  onSelectSubMask(id: string | null): void;
  onTogglePatchVisibility(id: string): void;
  selectedImage: SelectedImage;
  setAdjustments(updater: any): void;
  setBrushSettings(brushSettings: BrushSettings | null): void;
  setCustomEscapeHandler(handler: any): void;
  onDragStateChange?: (isDragging: boolean) => void;
}

interface ConnectionStatusProps {
  isConnected: boolean;
}

interface DragData {
  type: 'Container' | 'SubMask' | 'Creation';
  item?: AiPatch | SubMask;
  maskType?: Mask;
  parentId?: string;
}

function formatMaskTypeName(type: string, t: any) {
  if (type === Mask.AiSubject) return t('masking.aiSubject');
  if (type === Mask.AiForeground) return t('masking.aiForeground');
  if (type === Mask.AiSky) return t('masking.aiSky');
  return type.charAt(0).toUpperCase() + type.slice(1);
}

const PLACEHOLDER_PATCH: AiPatch = {
  id: 'placeholder',
  invert: false,
  isLoading: false,
  name: '',
  patchData: null,
  prompt: '',
  subMasks: [],
  visible: true,
};

const getSubMaskConfig = (t: any): any => ({
  [Mask.Radial]: {
    parameters: [
      { key: 'feather', label: t('ai.feather'), min: 0, max: 100, step: 1, multiplier: 100, defaultValue: 50 },
    ],
  },
  [Mask.Brush]: { showBrushTools: true },
  [Mask.Linear]: { parameters: [] },
  [Mask.AiSubject]: {
    parameters: [
      { key: 'grow', label: t('ai.grow'), min: -100, max: 100, step: 1, defaultValue: 50 },
      { key: 'feather', label: t('ai.feather'), min: 0, max: 100, step: 1, defaultValue: 25 },
    ],
  },
  [Mask.AiForeground]: {
    parameters: [
      { key: 'grow', label: t('ai.grow'), min: -100, max: 100, step: 1, defaultValue: 50 },
      { key: 'feather', label: t('ai.feather'), min: 0, max: 100, step: 1, defaultValue: 25 },
    ],
  },
  [Mask.AiSky]: {
    parameters: [
      { key: 'grow', label: t('ai.grow'), min: -100, max: 100, step: 1, defaultValue: 0 },
      { key: 'feather', label: t('ai.feather'), min: 0, max: 100, step: 1, defaultValue: 0 },
    ],
  },
  [Mask.QuickEraser]: {
    parameters: [
      { key: 'grow', label: t('ai.grow'), min: -100, max: 100, step: 1, defaultValue: 50 },
      { key: 'feather', label: t('ai.feather'), min: 0, max: 100, step: 1, defaultValue: 50 },
    ],
  },
});

const BrushTools = ({ settings, onSettingsChange }: { settings: any; onSettingsChange: any }) => {
  const { t } = useTranslation();
  return (
    <div className="space-y-4 pt-4 border-t border-surface mt-4">
      <Slider
        defaultValue={100}
        label={t('masking.brushSize')}
        max={200}
        min={1}
        onChange={(e: any) => onSettingsChange((s: any) => ({ ...s, size: Number(e.target.value) }))}
        step={1}
        value={settings.size}
      />
      <Slider
        defaultValue={50}
        label={t('masking.brushFeather')}
        max={100}
        min={0}
        onChange={(e: any) => onSettingsChange((s: any) => ({ ...s, feather: Number(e.target.value) }))}
        step={1}
        value={settings.feather}
      />
      <div className="grid grid-cols-2 gap-2 pt-2">
        <button
          className={`p-2 rounded-md text-sm font-medium transition-colors flex items-center justify-center gap-2 ${
            settings.tool === ToolType.Brush
              ? 'text-primary bg-surface'
              : 'bg-surface text-text-secondary hover:bg-card-active'
          }`}
          onClick={() => onSettingsChange((s: any) => ({ ...s, tool: ToolType.Brush }))}
        >
          {t('common.add')}
        </button>
        <button
          className={`p-2 rounded-md text-sm font-medium transition-colors flex items-center justify-center gap-2 ${
            settings.tool === ToolType.Eraser
              ? 'text-primary bg-surface'
              : 'bg-surface text-text-secondary hover:bg-card-active'
          }`}
          onClick={() => onSettingsChange((s: any) => ({ ...s, tool: ToolType.Eraser }))}
        >
          {t('common.erase')}
        </button>
      </div>
    </div>
  );
};

const ConnectionStatus = ({ isConnected }: ConnectionStatusProps) => {
  const [isHovered, setIsHovered] = useState(false);
  const { t } = useTranslation();
  if (isConnected) {
    return (
      <div className="flex items-center gap-2 px-4 py-2 bg-surface rounded-lg mb-4">
        <div className={'w-2.5 h-2.5 rounded-full bg-green-500'} />
        <span className="text-sm font-medium text-text-secondary">{t('ai.aiConnectorLabel')}</span>
        <span className={'text-sm font-bold text-green-400'}>{t('ai.aiConnectorReady')}</span>
      </div>
    );
  }
  return (
    <div
      className="bg-surface rounded-lg mb-4"
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      <div className="flex items-center gap-2 px-4 pt-2">
        <div className={'w-2.5 h-2.5 rounded-full bg-red-500'} />
        <span className="text-sm font-medium text-text-secondary">{t('ai.aiConnectorLabel')}</span>
        <span className={'text-sm font-bold text-red-400'}>{t('ai.aiConnectorNotDetected')}</span>
      </div>
      <div className="px-4 pb-2">
        <motion.div
          animate={{ height: isHovered ? 'auto' : 0, opacity: isHovered ? 1 : 0, marginTop: isHovered ? '2px' : 0 }}
          className="overflow-hidden"
          initial={{ height: 0, opacity: 0, marginTop: 0 }}
          transition={{ duration: 0.2, ease: 'easeInOut' }}
        >
          <p className="text-xs text-text-secondary">{t('ai.simpleInpaintingOnly')}</p>
        </motion.div>
      </div>
    </div>
  );
};

export default function AIPanel({
  adjustments,
  setAdjustments,
  selectedImage,
  isAIConnectorConnected,
  isGeneratingAi,
  onGenerativeReplace,
  onDeletePatch,
  onTogglePatchVisibility: _onTogglePatchVisibility,
  activePatchContainerId,
  onSelectPatchContainer,
  activeSubMaskId,
  onSelectSubMask,
  brushSettings,
  setBrushSettings,
  isGeneratingAiMask,
  aiModelDownloadStatus,
  onGenerateAiForegroundMask,
  setCustomEscapeHandler,
  onDragStateChange,
}: AiPanelProps) {
  const [expandedContainers, setExpandedContainers] = useState<Set<string>>(new Set());
  const { t } = useTranslation();
  const [activeDragItem, setActiveDragItem] = useState<DragData | null>(null);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [tempName, setTempName] = useState('');
  const [isPatchListEmpty, setIsPatchListEmpty] = useState((adjustments.aiPatches || []).length === 0);
  const [pendingAction, setPendingAction] = useState<(() => void) | null>(null);
  const [isSettingsPanelEverOpened, setIsSettingsPanelEverOpened] = useState(false);
  const hasPerformedInitialSelection = useRef(false);
  const [analyzingSubMaskId, setAnalyzingSubMaskId] = useState<string | null>(null);

  const [collapsibleState, setCollapsibleState] = useState({
    generative: true,
    properties: true,
  });

  const { setNodeRef: setRootDroppableRef, isOver: isRootOver } = useDroppable({ id: 'ai-list-root' });
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));

  const activeContainer = (adjustments.aiPatches || []).find((p) => p.id === activePatchContainerId);
  const activeSubMaskData = activeContainer?.subMasks.find((sm) => sm.id === activeSubMaskId);
  const isAiMask =
    activeSubMaskData && [Mask.AiSubject, Mask.AiForeground, Mask.AiSky].includes(activeSubMaskData.type);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | null = null;
    if (isGeneratingAiMask && isAiMask) {
      timer = setTimeout(() => {
        setAnalyzingSubMaskId(activeSubMaskId);
      }, 200);
    } else {
      setAnalyzingSubMaskId(null);
    }
    return () => {
      if (timer) clearTimeout(timer);
    };
  }, [isGeneratingAiMask, isAiMask, activeSubMaskId]);

  useEffect(() => {
    if (activePatchContainerId) {
      const patchExists = adjustments.aiPatches?.some((p) => p.id === activePatchContainerId);
      if (!patchExists) {
        onSelectPatchContainer(null);
        onSelectSubMask(null);
      }
    }
  }, [adjustments.aiPatches, activePatchContainerId, onSelectPatchContainer, onSelectSubMask]);

  useEffect(() => {
    const hasPatches = (adjustments.aiPatches || []).length > 0;

    if (hasPatches) {
      setIsSettingsPanelEverOpened(true);
      setIsPatchListEmpty(false);
    }

    if (activePatchContainerId) {
      const shouldAutoExpand = !hasPerformedInitialSelection.current || activeSubMaskId;
      if (shouldAutoExpand) {
        setExpandedContainers((prev) => {
          if (prev.has(activePatchContainerId)) return prev;
          return new Set(prev).add(activePatchContainerId);
        });
      }
      hasPerformedInitialSelection.current = true;
      setIsSettingsPanelEverOpened(true);
    }
  }, [activePatchContainerId, activeSubMaskId, adjustments.aiPatches, onSelectPatchContainer, onSelectSubMask]);

  useEffect(() => {
    const handler = () => {
      if (renamingId) {
        setRenamingId(null);
        setTempName('');
      } else if (activeSubMaskId) onSelectSubMask(null);
      else if (activePatchContainerId) onSelectPatchContainer(null);
    };
    if (activePatchContainerId || renamingId) setCustomEscapeHandler(() => handler);
    else setCustomEscapeHandler(null);
    return () => setCustomEscapeHandler(null);
  }, [
    activePatchContainerId,
    activeSubMaskId,
    renamingId,
    onSelectPatchContainer,
    onSelectSubMask,
    setCustomEscapeHandler,
  ]);

  const handleDeselect = () => {
    onSelectPatchContainer(null);
    onSelectSubMask(null);
  };

  const handleToggleExpand = (id: string) => {
    setExpandedContainers((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleResetAllAiEdits = () => {
    if (isGeneratingAi) return;
    handleDeselect();
    setAdjustments((prev: Adjustments) => ({ ...prev, aiPatches: [] }));
  };

  const createMaskLogic = (type: Mask) => {
    const subMask = createSubMask(type, selectedImage);

    const steps = adjustments?.orientationSteps || 0;
    const isRotated = steps === 1 || steps === 3;
    const imgW = isRotated ? selectedImage.height || 1000 : selectedImage.width || 1000;
    const imgH = isRotated ? selectedImage.width || 1000 : selectedImage.height || 1000;

    const config = getSubMaskConfig(t)[type];
    if (config && config.parameters) {
      config.parameters.forEach((param: any) => {
        if (param.defaultValue !== undefined) {
          subMask.parameters[param.key] = param.defaultValue / (param.multiplier || 1);
        }
      });
    }

    if (type === Mask.Linear && subMask.parameters) {
      subMask.parameters.range = Math.min(imgW, imgH) * 0.1;
    }

    if (type === Mask.Linear || type === Mask.Radial) {
      if (!subMask.parameters) subMask.parameters = {};
      subMask.parameters.isInitialDraw = true;
      subMask.parameters.startX = -10000;
      subMask.parameters.startY = -10000;
      subMask.parameters.endX = -10000;
      subMask.parameters.endY = -10000;
      subMask.parameters.centerX = -10000;
      subMask.parameters.centerY = -10000;
      subMask.parameters.radiusX = 0;
      subMask.parameters.radiusY = 0;
    } else if (adjustments?.crop && subMask.parameters) {
      const { x, y, width, height, unit } = adjustments.crop as any;
      const isPercent = unit === '%';

      const cW = isPercent ? (width / 100) * imgW : width;
      const cH = isPercent ? (height / 100) * imgH : height;
      const cX = isPercent ? (x / 100) * imgW : x;
      const cY = isPercent ? (y / 100) * imgH : y;

      if (imgW && imgH) {
        const _ratioX = cW / imgW;
        const _ratioY = cH / imgH;
        const _cx = cX + cW / 2;
        const _cy = cY + cH / 2;
        const _ox = imgW / 2;
        const _oy = imgH / 2;

        const p = { ...subMask.parameters };
        subMask.parameters = p;
      }
    }
    return subMask;
  };

  const handleAddAiPatchContainer = (type: Mask) => {
    if ((adjustments.aiPatches || []).length === 0) setIsPatchListEmpty(false);
    const subMask = createMaskLogic(type);

    let name: string;
    if (type === Mask.QuickEraser) {
      const count =
        (adjustments.aiPatches || []).filter((p: AiPatch) =>
          p.subMasks.some((sm: SubMask) => sm.type === Mask.QuickEraser),
        ).length + 1;
      name = t('ai.quickEraseCount', { count });
    } else {
      name = t('ai.aiEditCount', { count: (adjustments.aiPatches || []).length + 1 });
    }

    const newContainer: AiPatch = {
      id: uuidv4(),
      invert: false,
      isLoading: false,
      name: name,
      patchData: null,
      prompt: '',
      subMasks: [subMask],
      visible: true,
    };

    setAdjustments((prev: Adjustments) => ({ ...prev, aiPatches: [...(prev.aiPatches || []), newContainer] }));
    onSelectPatchContainer(newContainer.id);
    onSelectSubMask(subMask.id);
    setExpandedContainers((prev) => new Set(prev).add(newContainer.id));

    if (type === Mask.AiForeground) onGenerateAiForegroundMask(subMask.id);
  };

  const handleAddSubMask = (containerId: string, type: Mask, insertIndex: number = -1) => {
    const subMask = createMaskLogic(type);
    setAdjustments((prev: Adjustments) => ({
      ...prev,
      aiPatches: prev.aiPatches?.map((c: AiPatch) => {
        if (c.id === containerId) {
          const newSubMasks = [...c.subMasks];
          if (insertIndex >= 0) newSubMasks.splice(insertIndex, 0, subMask);
          else newSubMasks.push(subMask);
          return { ...c, subMasks: newSubMasks };
        }
        return c;
      }),
    }));
    onSelectPatchContainer(containerId);
    onSelectSubMask(subMask.id);
    setExpandedContainers((prev) => new Set(prev).add(containerId));
    if (type === Mask.AiForeground) onGenerateAiForegroundMask(subMask.id);
  };

  const updatePatch = (id: string, data: any) =>
    setAdjustments((prev: Adjustments) => ({
      ...prev,
      aiPatches: prev.aiPatches.map((p) => (p.id === id ? { ...p, ...data } : p)),
    }));

  const updateSubMask = (id: string, data: any) =>
    setAdjustments((prev: Adjustments) => ({
      ...prev,
      aiPatches: prev.aiPatches.map((p) => ({
        ...p,
        subMasks: p.subMasks.map((sm) => (sm.id === id ? { ...sm, ...data } : sm)),
      })),
    }));

  const handleDeleteContainer = (id: string) => {
    if (activePatchContainerId === id) handleDeselect();
    onDeletePatch(id);
  };

  const handleDeleteSubMask = (containerId: string, subMaskId: string) => {
    if (activeSubMaskId === subMaskId) onSelectSubMask(null);
    setAdjustments((prev: Adjustments) => ({
      ...prev,
      aiPatches: prev.aiPatches.map((p) =>
        p.id === containerId ? { ...p, subMasks: p.subMasks.filter((sm) => sm.id !== subMaskId) } : p,
      ),
    }));
  };

  const handleDragStart = (event: DragStartEvent) => {
    setActiveDragItem(event.active.data.current as DragData);
    if (onDragStateChange) onDragStateChange(true);
  };

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    const dragData = active.data.current as DragData;
    const overData = over?.data.current as DragData;

    setActiveDragItem(null);
    if (onDragStateChange) onDragStateChange(false);

    if (dragData.type === 'Creation' && dragData.maskType) {
      const creationFn = () => {
        if (overData?.type === 'Container') {
          handleAddSubMask(overData.item!.id, dragData.maskType!);
        } else if (overData?.type === 'SubMask') {
          const container = adjustments.aiPatches.find((p) => p.id === overData.parentId);
          if (container) {
            const targetIndex = container.subMasks.findIndex((sm) => sm.id === over!.id);
            handleAddSubMask(overData.parentId!, dragData.maskType!, targetIndex);
          }
        } else {
          handleAddAiPatchContainer(dragData.maskType!);
        }
      };

      if (!isPatchListEmpty) setPendingAction(() => creationFn);
      else creationFn();
      return;
    }

    if (dragData.type === 'Container') {
      const overId = over?.id;
      if (!overId || active.id === overId) return;

      setAdjustments((prev: Adjustments) => {
        const oldIndex = prev.aiPatches.findIndex((p) => p.id === dragData.item!.id);
        let newIndex = -1;

        if (overId === 'ai-list-root') newIndex = prev.aiPatches.length - 1;
        else if (overData?.type === 'Container') newIndex = prev.aiPatches.findIndex((p) => p.id === overId);
        else if (overData?.type === 'SubMask') newIndex = prev.aiPatches.findIndex((p) => p.id === overData.parentId);

        if (oldIndex !== -1 && newIndex !== -1 && oldIndex !== newIndex) {
          const newPatches = [...prev.aiPatches];
          const [movedItem] = newPatches.splice(oldIndex, 1);
          newPatches.splice(newIndex, 0, movedItem);
          return { ...prev, aiPatches: newPatches };
        }
        return prev;
      });
      return;
    }

    if (dragData.type === 'SubMask') {
      const sourceContainerId = dragData.parentId;
      if (!sourceContainerId) return;

      if (over?.id === 'ai-list-root' || !over) {
        setAdjustments((prev: Adjustments) => {
          const newPatches = JSON.parse(JSON.stringify(prev.aiPatches));
          const sourceContainer = newPatches.find((p: AiPatch) => p.id === sourceContainerId);
          if (!sourceContainer) return prev;
          const subMaskIndex = sourceContainer.subMasks.findIndex((sm: SubMask) => sm.id === dragData.item!.id);
          if (subMaskIndex === -1) return prev;

          const [movedSubMask] = sourceContainer.subMasks.splice(subMaskIndex, 1);
          if ((adjustments.aiPatches || []).length === 0) setIsPatchListEmpty(false);

          const newContainer: AiPatch = {
            id: uuidv4(),
            invert: false,
            isLoading: false,
            name: t('ai.aiEditCount', { count: newPatches.length + 1 }),
            patchData: null,
            prompt: '',
            subMasks: [movedSubMask],
            visible: true,
          };
          newPatches.push(newContainer);

          setTimeout(() => {
            onSelectPatchContainer(newContainer.id);
            onSelectSubMask(movedSubMask.id);
            setExpandedContainers((p) => new Set(p).add(newContainer.id));
          }, 0);
          return { ...prev, aiPatches: newPatches };
        });
        return;
      }

      let targetContainerId: string | null = null;
      if (overData?.type === 'Container') targetContainerId = overData.item!.id;
      else if (overData?.type === 'SubMask') targetContainerId = overData.parentId || null;

      if (targetContainerId) {
        setAdjustments((prev: Adjustments) => {
          const newPatches = prev.aiPatches.map((p) => ({ ...p, subMasks: [...p.subMasks] }));
          const sourceContainer = newPatches.find((p) => p.id === sourceContainerId);
          const targetContainer = newPatches.find((p) => p.id === targetContainerId);
          if (!sourceContainer || !targetContainer) return prev;

          const sourceIndex = sourceContainer.subMasks.findIndex((sm) => sm.id === dragData.item!.id);
          if (sourceIndex === -1) return prev;
          const [movedSubMask] = sourceContainer.subMasks.splice(sourceIndex, 1);

          if (sourceContainerId === targetContainerId) {
            if (overData?.type === 'SubMask') {
              const overIndex = sourceContainer.subMasks.findIndex((sm) => sm.id === over.id);
              const insertIndex = overIndex >= 0 ? overIndex : sourceContainer.subMasks.length;
              sourceContainer.subMasks.splice(insertIndex, 0, movedSubMask);
            } else {
              sourceContainer.subMasks.push(movedSubMask);
            }
          } else {
            if (overData?.type === 'SubMask') {
              const overIndex = targetContainer.subMasks.findIndex((sm) => sm.id === over.id);
              const insertIndex = overIndex >= 0 ? overIndex : targetContainer.subMasks.length;
              targetContainer.subMasks.splice(insertIndex, 0, movedSubMask);
            } else {
              targetContainer.subMasks.push(movedSubMask);
            }
            setExpandedContainers((p) => new Set(p).add(targetContainerId!));
          }
          return { ...prev, aiPatches: newPatches };
        });
      }
    }
  };

  return (
    <DndContext
      sensors={sensors}
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
      collisionDetection={pointerWithin}
    >
      <div className="flex flex-col h-full select-none overflow-hidden" onClick={handleDeselect}>
        <div className="p-4 flex justify-between items-center flex-shrink-0 border-b border-surface h-[69px]">
          <h2 className="text-xl font-bold text-primary text-shadow-shiny">{t('ai.title')}</h2>
          <button
            className="p-2 rounded-full hover:bg-surface transition-colors"
            onClick={handleResetAllAiEdits}
            data-tooltip={t('ai.resetInpainting')}
          >
            <RotateCcw size={18} />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto overflow-x-hidden flex flex-col min-h-0">
          <div className="p-4 pb-2 z-10 flex-shrink-0">
            {!selectedImage && <p className="text-center text-text-tertiary mt-4">No image selected.</p>}

            {selectedImage && (
              <>
                <ConnectionStatus isConnected={isAIConnectorConnected} />
                <p className="text-sm mb-3 font-semibold text-text-primary">
                  {activePatchContainerId ? t('ai.addToSelection') : t('ai.createNewEdit')}
                </p>
                <div className="grid grid-cols-3 gap-2" onClick={(e) => e.stopPropagation()}>
                  {getAiPanelCreationTypes(t).map((maskType: MaskType) => {
                    const isComponentMode = !!activePatchContainerId;
                    const typeToRender = isComponentMode
                      ? getAiSubMaskComponentTypes(t).find((ct) => ct.type === maskType.type)
                      : maskType;

                    if (!typeToRender) return null;

                    return (
                      <DraggableGridItem
                        key={typeToRender.type}
                        maskType={typeToRender}
                        isGenerating={isGeneratingAi}
                        activePatchContainerId={activePatchContainerId}
                        onClick={() =>
                          typeToRender.type &&
                          (isComponentMode
                            ? handleAddSubMask(activePatchContainerId, typeToRender.type)
                            : handleAddAiPatchContainer(typeToRender.type))
                        }
                      />
                    );
                  })}
                </div>
              </>
            )}
          </div>

          <AnimatePresence>
            {isSettingsPanelEverOpened && selectedImage && (
              <motion.div
                ref={setRootDroppableRef}
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.2 }}
                className={`flex flex-col px-4 pb-2 space-y-1 transition-colors ${isRootOver ? 'bg-surface' : ''}`}
              >
                <p className="text-sm my-3 font-semibold text-text-primary">{t('ai.edits')}</p>

                {isPatchListEmpty && (adjustments.aiPatches || []).length === 0 && (
                  <div className="text-center text-text-secondary text-sm py-4 opacity-70">
                    {t('ai.noEditsCreated')}
                  </div>
                )}

                <AnimatePresence
                  initial={false}
                  mode="popLayout"
                  onExitComplete={() => {
                    if ((adjustments.aiPatches || []).length === 0) setIsPatchListEmpty(true);
                  }}
                >
                  {(adjustments.aiPatches || []).map((container) => (
                    <ContainerRow
                      key={container.id}
                      container={container}
                      isSelected={activePatchContainerId === container.id && activeSubMaskId === null}
                      hasActiveChild={activePatchContainerId === container.id && activeSubMaskId !== null}
                      isExpanded={expandedContainers.has(container.id)}
                      onToggle={() => handleToggleExpand(container.id)}
                      onSelect={() => {
                        onSelectPatchContainer(container.id);
                        onSelectSubMask(null);
                      }}
                      renamingId={renamingId}
                      setRenamingId={setRenamingId}
                      tempName={tempName}
                      setTempName={setTempName}
                      updateContainer={updatePatch}
                      handleDelete={handleDeleteContainer}
                      setAdjustments={setAdjustments}
                      activeDragItem={activeDragItem}
                      activeSubMaskId={activeSubMaskId}
                      onSelectContainer={onSelectPatchContainer}
                      onSelectSubMask={onSelectSubMask}
                      updateSubMask={updateSubMask}
                      handleDeleteSubMask={handleDeleteSubMask}
                      analyzingSubMaskId={analyzingSubMaskId}
                    />
                  ))}
                </AnimatePresence>

                <AnimatePresence
                  onExitComplete={() => {
                    if (pendingAction) {
                      pendingAction();
                      setPendingAction(null);
                    }
                  }}
                >
                  {activeDragItem?.type === 'Creation' && !isPatchListEmpty && <NewMaskDropZone isOver={isRootOver} />}
                </AnimatePresence>
              </motion.div>
            )}
          </AnimatePresence>

          <AnimatePresence>
            {isSettingsPanelEverOpened && (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.2, ease: 'easeOut' }}
                className="flex-1 min-h-0"
              >
                <p className="text-sm my-3 font-semibold text-text-primary px-4">{t('ai.editSettings')}</p>
                <SettingsPanel
                  container={activeContainer || null}
                  activeSubMask={activeSubMaskData || null}
                  aiModelDownloadStatus={aiModelDownloadStatus}
                  brushSettings={brushSettings}
                  setBrushSettings={setBrushSettings}
                  updateContainer={updatePatch}
                  updateSubMask={updateSubMask}
                  isAIConnectorConnected={isAIConnectorConnected}
                  isGeneratingAi={isGeneratingAi}
                  isGeneratingAiMask={isGeneratingAiMask}
                  onGenerativeReplace={onGenerativeReplace}
                  collapsibleState={collapsibleState}
                  setCollapsibleState={setCollapsibleState}
                />
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </div>

      <DragOverlay dropAnimation={{ duration: 150, easing: 'cubic-bezier(0.18, 0.67, 0.6, 1.22)' }}>
        {activeDragItem ? (
          <div className="w-[var(--sidebar-width,280px)] pointer-events-none">
            {activeDragItem.type === 'Container' && activeDragItem.item && (
              <div className="flex items-center gap-2 p-2 rounded-md bg-surface shadow-2xl opacity-90 ring-1 ring-black/10">
                <div className="text-text-secondary">
                  <Wand2 size={18} />
                </div>
                <span className="text-sm font-medium text-text-primary flex-1 truncate">
                  {(activeDragItem.item as AiPatch).name}
                </span>
              </div>
            )}
            {activeDragItem.type === 'SubMask' && activeDragItem.item && (
              <div className="flex items-center gap-2 p-2 rounded-md bg-surface shadow-2xl opacity-90 ring-1 ring-black/10 ml-[15px]">
                {(() => {
                  const sm = activeDragItem.item as SubMask;
                  const Icon = MASK_ICON_MAP[sm.type] || Circle;
                  return <Icon size={16} className="text-text-secondary flex-shrink-0 ml-1" />;
                })()}
                <span className="text-sm text-text-primary flex-1 truncate">
                  {formatMaskTypeName((activeDragItem.item as SubMask).type, t)}
                </span>
              </div>
            )}
            {activeDragItem.type === 'Creation' && (
              <div className="bg-surface text-text-primary rounded-lg p-2 flex flex-col items-center justify-center gap-1.5 aspect-square w-20 shadow-xl opacity-90">
                {(() => {
                  const maskType = getAiPanelCreationTypes(t).find((m) => m.type === activeDragItem.maskType);
                  const Icon = maskType?.icon || Circle;
                  return (
                    <>
                      <Icon size={24} />
                      <span className="text-xs text-center">
                        {activeDragItem.maskType ? formatMaskTypeName(activeDragItem.maskType, t) : t('panels.masks')}
                      </span>
                    </>
                  );
                })()}
              </div>
            )}
          </div>
        ) : null}
      </DragOverlay>
    </DndContext>
  );
}

function NewMaskDropZone({ isOver }: { isOver: boolean }) {
  return (
    <motion.div
      layout
      initial={{ opacity: 0, height: 0, marginTop: 0 }}
      animate={{ opacity: 1, height: 'auto', marginTop: '4px' }}
      exit={{ opacity: 0, height: 0, marginTop: 0 }}
      transition={{ duration: 0.2, ease: 'easeOut' }}
      className={`p-4 rounded-lg text-center ${isOver ? 'border border-accent/80 bg-bg-tertiary/50' : ''}`}
    >
      <p className="text-sm font-medium text-text-secondary">Drop here to create a new edit</p>
    </motion.div>
  );
}

function DraggableGridItem({ maskType, isGenerating, onClick, activePatchContainerId }: any) {
  const { attributes, listeners, setNodeRef, isDragging } = useDraggable({
    id: `create-ai-${maskType.type}`,
    data: { type: 'Creation', maskType: maskType.type },
    disabled: isGenerating,
  });
  return (
    <button
      ref={setNodeRef}
      {...listeners}
      {...attributes}
      disabled={maskType.disabled || isGenerating}
      onClick={onClick}
      className={`bg-surface text-text-primary rounded-lg p-2 flex flex-col items-center justify-center gap-1.5 aspect-square transition-colors
              ${maskType.disabled || isGenerating ? 'opacity-50 cursor-not-allowed' : 'hover:bg-card-active active:bg-accent/20'}
              ${isDragging ? 'opacity-50' : ''}`}
      data-tooltip={
        maskType.disabled
          ? 'Coming Soon'
          : activePatchContainerId
            ? `Add ${maskType.name} to Current Edit`
            : `Create New ${maskType.name} Edit`
      }
    >
      <maskType.icon size={24} /> <span className="text-xs">{maskType.name}</span>
    </button>
  );
}

function ContainerRow({
  container,
  isSelected,
  hasActiveChild,
  isExpanded,
  onToggle,
  onSelect,
  renamingId,
  setRenamingId,
  tempName,
  setTempName,
  updateContainer,
  handleDelete,
  activeDragItem,
  activeSubMaskId,
  onSelectContainer,
  onSelectSubMask,
  updateSubMask,
  handleDeleteSubMask,
  analyzingSubMaskId,
}: any) {
  const { setNodeRef: setDroppableRef, isOver } = useDroppable({
    id: container.id,
    data: { type: 'Container', item: container },
  });
  const {
    attributes,
    listeners,
    setNodeRef: setDraggableRef,
    isDragging,
  } = useDraggable({ id: container.id, data: { type: 'Container', item: container } });
  const [isSubMaskListEmpty, setIsSubMaskListEmpty] = useState(container.subMasks.length === 0);
  const { showContextMenu } = useContextMenu();
  const { t } = useTranslation();

  useEffect(() => {
    if (container.subMasks.length > 0 && isSubMaskListEmpty) setIsSubMaskListEmpty(false);
  }, [container.subMasks.length, isSubMaskListEmpty]);

  const setCombinedRef = (node: HTMLElement | null) => {
    setDroppableRef(node);
    setDraggableRef(node);
  };

  const handleRenameSubmit = () => {
    if (tempName.trim()) {
      updateContainer(container.id, { name: tempName.trim() });
    }
    setRenamingId(null);
  };

  const onContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    showContextMenu(e.clientX, e.clientY, [
      {
        label: t('ai.rename'),
        icon: FileEdit,
        onClick: () => {
          setRenamingId(container.id);
          setTempName(container.name);
        },
      },
      {
        label: t('ai.resetSelection'),
        icon: RotateCcw,
        onClick: () => updateContainer(container.id, { subMasks: [] }),
      },
      { label: t('ai.deleteEdit'), icon: Trash2, isDestructive: true, onClick: () => handleDelete(container.id) },
    ]);
  };

  const isDraggingContainer = activeDragItem?.type === 'Container';
  let borderClass = '';

  if (isOver) {
    if (isDraggingContainer) {
      borderClass = 'border-t-2 border-accent';
    } else if (
      (activeDragItem?.type === 'SubMask' && activeDragItem?.parentId !== container.id) ||
      activeDragItem?.type === 'Creation'
    ) {
      borderClass = 'bg-card-active border border-accent/50';
    }
  }

  return (
    <motion.div
      layout="position"
      initial={{ opacity: 0, height: 0 }}
      animate={{ opacity: isDragging ? 0.4 : 1, height: 'auto' }}
      exit={{ opacity: 0, scale: 0.95, transition: { duration: 0.2 } }}
      ref={setCombinedRef}
      className="mb-0.5 overflow-hidden"
    >
      <div
        {...listeners}
        {...attributes}
        className={`flex items-center gap-2 p-2 rounded-md transition-colors group
             ${isSelected ? 'bg-surface' : 'hover:bg-card-active'}
             ${borderClass}`}
        onClick={(e) => {
          e.stopPropagation();
          onSelect();
        }}
        onContextMenu={onContextMenu}
      >
        <div
          onClick={(e) => {
            e.stopPropagation();
            onToggle();
          }}
          className={`p-0.5 rounded transition-colors cursor-pointer ${
            hasActiveChild ? 'text-text-primary' : isExpanded ? 'text-primary' : 'text-text-secondary'
          }`}
        >
          {isExpanded ? <FolderOpen size={18} /> : <Wand2 size={18} />}
        </div>
        <div
          className="flex-1 min-w-0 cursor-pointer"
          onDoubleClick={(e) => {
            e.stopPropagation();
            onToggle();
          }}
        >
          {renamingId === container.id ? (
            <input
              autoFocus
              className="bg-bg-primary text-sm w-full rounded px-1 outline-none border border-accent"
              value={tempName}
              onChange={(e) => setTempName(e.target.value)}
              onBlur={handleRenameSubmit}
              onKeyDown={(e) => e.key === 'Enter' && handleRenameSubmit()}
              onClick={(e) => e.stopPropagation()}
            />
          ) : (
            <span
              className={`text-sm font-medium truncate select-none ${
                isSelected ? 'text-primary' : 'text-text-primary'
              } ${hasActiveChild ? 'text-text-primary font-bold' : ''}`}
            >
              {container.name}
            </span>
          )}
        </div>
        <div className="flex gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
          <button
            className="p-1 hover:text-text-primary text-text-secondary"
            data-tooltip={container.visible ? 'Hide Edit' : 'Show Edit'}
            onClick={(e) => {
              e.stopPropagation();
              updateContainer(container.id, { visible: !container.visible });
            }}
          >
            {container.visible ? <Eye size={16} /> : <EyeOff size={16} />}
          </button>
          <button
            className="p-1 hover:text-red-500 text-text-secondary"
            data-tooltip={t('ai.deleteEdit')}
            onClick={(e) => {
              e.stopPropagation();
              handleDelete(container.id);
            }}
          >
            <Trash2 size={16} />
          </button>
        </div>
      </div>

      <AnimatePresence initial={false}>
        {isExpanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            className="overflow-hidden pl-2 border-l border-border-color/20 ml-[15px]"
            layout
          >
            <AnimatePresence
              mode="popLayout"
              initial={false}
              onExitComplete={() => {
                if (container.subMasks.length === 0) setIsSubMaskListEmpty(true);
              }}
            >
              {container.subMasks.map((subMask: SubMask, index: number) => (
                <SubMaskRow
                  key={subMask.id}
                  subMask={subMask}
                  index={index + 1}
                  totalCount={container.subMasks.length}
                  containerId={container.id}
                  isActive={activeSubMaskId === subMask.id}
                  parentVisible={container.visible}
                  activeDragItem={activeDragItem}
                  onSelect={() => {
                    onSelectContainer(container.id);
                    onSelectSubMask(subMask.id);
                  }}
                  updateSubMask={updateSubMask}
                  handleDelete={() => handleDeleteSubMask(container.id, subMask.id)}
                  analyzingSubMaskId={analyzingSubMaskId}
                  isParentLoading={container.isLoading}
                />
              ))}
            </AnimatePresence>
            {isSubMaskListEmpty && (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="p-3 text-xs text-text-secondary text-center italic"
              >
                No selection components.
              </motion.div>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  );
}

function SubMaskRow({
  subMask,
  index,
  totalCount,
  containerId,
  isActive,
  parentVisible,
  onSelect,
  updateSubMask,
  handleDelete,
  activeDragItem,
  analyzingSubMaskId,
  isParentLoading,
}: any) {
  const { attributes, listeners, setNodeRef, isDragging } = useDraggable({
    id: subMask.id,
    data: { type: 'SubMask', item: subMask, parentId: containerId },
  });
  const { setNodeRef: setDroppableRef, isOver } = useDroppable({
    id: subMask.id,
    data: { type: 'SubMask', item: subMask, parentId: containerId },
  });
  const setCombinedRef = (node: HTMLElement | null) => {
    setNodeRef(node);
    setDroppableRef(node);
  };
  const MaskIcon = MASK_ICON_MAP[subMask.type as Mask] || Circle;
  const { showContextMenu } = useContextMenu();
  const { t } = useTranslation();
  const [isHovered, setIsHovered] = useState(false);
  const hoverTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isDraggingContainer = activeDragItem?.type === 'Container';
  const isAnalyzing = subMask.id === analyzingSubMaskId || (isParentLoading && subMask.type === Mask.QuickEraser);

  const handleMouseEnter = () => {
    if (hoverTimeoutRef.current) {
      clearTimeout(hoverTimeoutRef.current);
      hoverTimeoutRef.current = null;
    }
    setIsHovered(true);
  };
  const handleMouseLeave = () => {
    hoverTimeoutRef.current = setTimeout(() => setIsHovered(false), 1000);
  };
  useEffect(() => {
    return () => {
      if (hoverTimeoutRef.current) clearTimeout(hoverTimeoutRef.current);
    };
  }, []);

  const onContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    showContextMenu(e.clientX, e.clientY, [
      { label: t('ai.deleteComponent'), icon: Trash2, isDestructive: true, onClick: handleDelete },
    ]);
  };
  const showNumber = isHovered && totalCount > 1;

  return (
    <motion.div
      layout="position"
      initial={{ opacity: 0, x: -15 }}
      animate={{ opacity: 1, x: 0, scale: 1 }}
      exit={{ opacity: 0, x: -15, scale: 0.95, transition: { duration: 0.2 } }}
      ref={setCombinedRef}
      {...attributes}
      {...listeners}
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      className={`flex items-center gap-2 p-2 rounded-md transition-colors group mt-0.5 cursor-pointer
            ${isActive ? 'bg-surface' : 'hover:bg-card-active'}
            ${isOver && !isDraggingContainer ? 'border-t-2 border-accent' : ''}
            ${isDragging ? 'opacity-40 z-50' : ''}
            ${parentVisible === false ? 'opacity-50' : ''}
            ${isDraggingContainer ? 'opacity-30 pointer-events-none' : ''}
            transition-opacity duration-300`}
      onClick={(e) => {
        e.stopPropagation();
        onSelect();
      }}
      onContextMenu={onContextMenu}
    >
      <div className="relative w-4 h-4 ml-1 flex-shrink-0 flex items-center justify-center">
        <AnimatePresence mode="wait" initial={false}>
          {isAnalyzing ? (
            <motion.div
              key="analyzing"
              initial={{ opacity: 0, scale: 0.5 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.5 }}
              transition={{ duration: 0.15 }}
              className="absolute"
            >
              <Loader2 size={16} className="text-text-secondary animate-spin" />
            </motion.div>
          ) : showNumber ? (
            <motion.span
              key="number"
              initial={{ opacity: 0, scale: 0.5 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.5 }}
              transition={{ duration: 0.15 }}
              className="text-xs font-bold text-text-secondary absolute"
            >
              {index}
            </motion.span>
          ) : (
            <motion.div
              key="icon"
              initial={{ opacity: 0, scale: 0.5 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.5 }}
              transition={{ duration: 0.15 }}
              className="absolute"
            >
              <MaskIcon size={16} className="text-text-secondary" />
            </motion.div>
          )}
        </AnimatePresence>
      </div>
      <span className="text-sm text-text-primary flex-1 truncate select-none">
        {formatMaskTypeName(subMask.type, t)}
      </span>
      <div className="flex gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
        <button
          className="p-1 hover:bg-bg-primary rounded text-text-secondary"
          data-tooltip={subMask.mode === SubMaskMode.Additive ? 'Switch to Subtract' : 'Switch to Add'}
          onClick={(e) => {
            e.stopPropagation();
            updateSubMask(subMask.id, {
              mode: subMask.mode === SubMaskMode.Additive ? SubMaskMode.Subtractive : SubMaskMode.Additive,
            });
          }}
        >
          {subMask.mode === SubMaskMode.Additive ? <Plus size={14} /> : <Minus size={14} />}
        </button>
        <button
          className="p-1 hover:text-red-500 text-text-secondary"
          data-tooltip={t('ai.deleteComponent')}
          onClick={(e) => {
            e.stopPropagation();
            handleDelete();
          }}
        >
          <Trash2 size={14} />
        </button>
      </div>
    </motion.div>
  );
}

function SettingsPanel({
  container,
  activeSubMask,
  aiModelDownloadStatus,
  brushSettings,
  setBrushSettings,
  updateContainer,
  updateSubMask,
  isAIConnectorConnected,
  isGeneratingAi,
  isGeneratingAiMask: _isGeneratingAiMask,
  onGenerativeReplace,
  collapsibleState,
  setCollapsibleState,
}: any) {
  const isActive = !!container;
  const isComponentMode = !!activeSubMask;
  const { t } = useTranslation();

  const displayContainer = container || PLACEHOLDER_PATCH;

  const [prompt, setPrompt] = useState(displayContainer.prompt || '');
  const [useFastInpaint, setUseFastInpaint] = useState(!isAIConnectorConnected);

  useEffect(() => {
    if (container) setPrompt(container.prompt || '');
  }, [container?.id]);

  const isQuickErasePatch = displayContainer.subMasks?.some((sm: SubMask) => sm.type === Mask.QuickEraser);
  useEffect(() => {
    if (container) {
      setUseFastInpaint(isQuickErasePatch || !isAIConnectorConnected);
    }
  }, [isAIConnectorConnected, container, isQuickErasePatch]);

  const subMaskConfig = activeSubMask ? getSubMaskConfig(t)[activeSubMask.type as Mask] || {} : {};
  const isAiMask =
    activeSubMask &&
    (activeSubMask.type === Mask.AiSubject ||
      activeSubMask.type === Mask.AiForeground ||
      activeSubMask.type === Mask.AiSky);

  const handleGenerateClick = () => {
    if (!container) return;
    updateContainer(container.id, { prompt });
    onGenerativeReplace(container.id, prompt, useFastInpaint);
  };

  const handleToggleSection = (section: string) =>
    setCollapsibleState((prev: any) => ({ ...prev, [section]: !prev[section] }));

  return (
    <div
      className={`px-4 pb-4 space-y-2 transition-opacity duration-300 ${
        !isActive ? 'opacity-50 pointer-events-none' : ''
      }`}
      onClick={(e) => e.stopPropagation()}
    >
      <CollapsibleSection
        title={t('ai.generativeReplace', 'Generative Replace')}
        isOpen={collapsibleState.generative}
        onToggle={() => handleToggleSection('generative')}
        canToggleVisibility={false}
        isContentVisible={true}
      >
        <div className="space-y-3 pt-2">
          <p className="text-xs text-text-secondary">
            {isQuickErasePatch
              ? t('ai.fillSelectionRemove', 'Fill selection to remove the object.')
              : useFastInpaint
                ? t('ai.fillSelectionSurrounding', 'Fill selection based on surrounding pixels.')
                : t('ai.describeGeneration')}
          </p>

          <Switch
            checked={useFastInpaint}
            disabled={isQuickErasePatch || !isAIConnectorConnected}
            label={t('ai.useFastInpainting')}
            onChange={setUseFastInpaint}
            tooltip={
              isQuickErasePatch
                ? t('ai.quickEraseAlwaysFast')
                : !isAIConnectorConnected
                  ? t('ai.aiConnectorNotConnectedFast')
                  : t('ai.fastInpaintingTooltip')
            }
          />

          <AnimatePresence>
            {!useFastInpaint && (
              <motion.div
                animate={{ opacity: 1, height: 'auto', marginTop: '0.75rem' }}
                className="overflow-hidden"
                exit={{ opacity: 0, height: 0, marginTop: 0 }}
                initial={{ opacity: 0, height: 0, marginTop: 0 }}
                transition={{ duration: 0.2 }}
              >
                <div className="flex items-center gap-2">
                  <Input
                    className="flex-grow"
                    disabled={isGeneratingAi || displayContainer.isLoading}
                    onChange={(e: any) => {
                      setPrompt(e.target.value);
                    }}
                    onBlur={() => isActive && updateContainer(container.id, { prompt })}
                    onKeyDown={(e: any) => {
                      if (e.key === 'Enter') handleGenerateClick();
                    }}
                    placeholder="e.g., a field of flowers"
                    type="text"
                    value={prompt}
                  />
                </div>
              </motion.div>
            )}
          </AnimatePresence>

          <Button
            className="w-full"
            disabled={isGeneratingAi || displayContainer.isLoading || displayContainer.subMasks.length === 0}
            onClick={handleGenerateClick}
          >
            {isGeneratingAi || displayContainer.isLoading ? (
              <Loader2 size={16} className="animate-spin" />
            ) : (
              <Send size={16} />
            )}
            <span className="ml-2">
              {isGeneratingAi || displayContainer.isLoading
                ? 'Generating...'
                : useFastInpaint
                  ? 'Inpaint Selection'
                  : 'Generate with AI'}
            </span>
          </Button>
        </div>
      </CollapsibleSection>

      <CollapsibleSection
        title={isComponentMode ? `${formatMaskTypeName(activeSubMask.type, t)} Properties` : 'Selection Properties'}
        isOpen={collapsibleState.properties}
        onToggle={() => handleToggleSection('properties')}
        canToggleVisibility={false}
        isContentVisible={true}
      >
        <div className="space-y-4 pt-2">
          <Switch
            checked={!!(isComponentMode ? activeSubMask.invert : displayContainer.invert)}
            label={isComponentMode ? 'Invert Component' : 'Invert Selection'}
            onChange={(v) =>
              isComponentMode
                ? updateSubMask(activeSubMask.id, { invert: v })
                : updateContainer(container.id, { invert: v })
            }
          />

          {isComponentMode && (
            <>
              {isAiMask && aiModelDownloadStatus && (
                <div className="p-3 mb-4 bg-card-active rounded-md border border-surface flex items-center gap-3">
                  <Loader2 size={16} className="text-accent animate-spin flex-shrink-0" />
                  <div className="text-xs text-text-secondary leading-relaxed">
                    AI Model Downloading: <span className="text-accent font-medium">{aiModelDownloadStatus}</span>
                  </div>
                </div>
              )}

              {subMaskConfig.parameters?.map((param: any) => (
                <Slider
                  key={param.key}
                  label={param.label}
                  min={param.min}
                  max={param.max}
                  step={param.step}
                  defaultValue={param.defaultValue}
                  value={(activeSubMask.parameters[param.key] || 0) * (param.multiplier || 1)}
                  onChange={(e: any) =>
                    updateSubMask(activeSubMask.id, {
                      parameters: {
                        ...activeSubMask.parameters,
                        [param.key]: parseFloat(e.target.value) / (param.multiplier || 1),
                      },
                    })
                  }
                />
              ))}

              {subMaskConfig.showBrushTools && brushSettings && (
                <BrushTools settings={brushSettings} onSettingsChange={setBrushSettings} />
              )}
            </>
          )}
        </div>
      </CollapsibleSection>
    </div>
  );
}
