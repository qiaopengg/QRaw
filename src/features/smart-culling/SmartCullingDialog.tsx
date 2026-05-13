import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-shell';
import { Download, FolderOpen, Loader2, Save, Sparkles, Square, Trash2, TriangleAlert } from 'lucide-react';
import Button from '../../components/ui/Button';
import Dropdown from '../../components/ui/Dropdown';
import Switch from '../../components/ui/Switch';
import Text from '../../components/ui/Text';
import { Invokes, ImageFile } from '../../components/ui/AppProperties';
import { useUIStore } from '../../store/useUIStore';
import { TextColors, TextVariants, TextWeights } from '../../types/typography';
import {
  SMART_CULLING_AESTHETIC_PREFERENCES,
  SMART_CULLING_FACE_CHECKS,
  SMART_CULLING_MODES,
  SMART_CULLING_PRESETS,
  SMART_CULLING_RANGES,
  SMART_CULLING_REVIEW_VIEW,
} from './constants';
import { useSmartCullingStore } from './useSmartCulling';
import type {
  SmartCullingAestheticPreference,
  SmartCullingFaceCheck,
  SmartCullingHistoryItem,
  SmartCullingMode,
  SmartCullingModelsStatus,
  SmartCullingPreset,
  SmartCullingPresetConfig,
  SmartCullingRange,
  SmartCullingSavePresetParams,
  SmartCullingStartParams,
  SmartCullingTaskResult,
  SmartCullingUserPreset,
} from './types';

interface SmartCullingDialogProps {
  currentFolderPath: string | null;
  imageList: ImageFile[];
  allImageList?: ImageFile[];
  selectedPaths: string[];
}

export default function SmartCullingDialog({
  currentFolderPath,
  imageList,
  allImageList,
  selectedPaths,
}: SmartCullingDialogProps) {
  const { activeTaskId, dialogOpen, isRunning, progress, setSmartCulling } = useSmartCullingStore();
  const [modelsStatus, setModelsStatus] = useState<SmartCullingModelsStatus | null>(null);
  const [range, setRange] = useState<SmartCullingRange>('current_folder');
  const [mode, setMode] = useState<SmartCullingMode>('general');
  const [preset, setPreset] = useState<SmartCullingPreset>('balanced');
  const [aestheticPreference, setAestheticPreference] = useState<SmartCullingAestheticPreference>('general');
  const [includeEdited, setIncludeEdited] = useState(false);
  const [previewOnly, setPreviewOnly] = useState(false);
  const [faceAnalysisEnabled, setFaceAnalysisEnabled] = useState(false);
  const [faceChecks, setFaceChecks] = useState<SmartCullingFaceCheck[]>([
    'closed_eyes',
    'blurred_face',
    'abnormal_expression',
    'smile',
    'best_group_expression',
    'looking_camera',
  ]);
  const [keepPerGroup, setKeepPerGroup] = useState(1);
  const [isStarting, setIsStarting] = useState(false);
  const [isDownloadingModels, setIsDownloadingModels] = useState(false);
  const [isSavingPreset, setIsSavingPreset] = useState(false);
  const [isDeletingPreset, setIsDeletingPreset] = useState(false);
  const [isCancellingTask, setIsCancellingTask] = useState(false);
  const [recentTasks, setRecentTasks] = useState<SmartCullingHistoryItem[]>([]);
  const [userPresets, setUserPresets] = useState<SmartCullingUserPreset[]>([]);
  const [selectedUserPresetId, setSelectedUserPresetId] = useState('');
  const [presetName, setPresetName] = useState('');
  const setUI = useUIStore((state) => state.setUI);

  const currentPresetConfig = useMemo<SmartCullingPresetConfig>(
    () => ({
      mode,
      preset,
      aestheticPreference,
      includeEdited,
      previewOnly,
      keepPerGroup,
      faceAnalysisEnabled,
      faceChecks,
    }),
    [aestheticPreference, faceAnalysisEnabled, faceChecks, includeEdited, keepPerGroup, mode, preset, previewOnly],
  );

  const defaultPresetName = useMemo(() => {
    const modeLabel = SMART_CULLING_MODES.find((item) => item.value === mode)?.label || '通用模式';
    const presetLabel = SMART_CULLING_PRESETS.find((item) => item.value === preset)?.label || '均衡筛选';
    return `${modeLabel} · ${presetLabel}`;
  }, [mode, preset]);

  const paths = useMemo(() => {
    if (range === 'selected') return selectedPaths;
    if (range === 'current_folder') return (allImageList ?? imageList).map((image) => image.path);
    return imageList.map((image) => image.path);
  }, [allImageList, imageList, range, selectedPaths]);

  useEffect(() => {
    if (!dialogOpen) return;
    refreshModelsStatus();
    refreshRecentTasks();
    refreshUserPresets();
  }, [dialogOpen]);

  useEffect(() => {
    if (!isRunning) setIsCancellingTask(false);
  }, [isRunning]);

  if (!dialogOpen) return null;

  const canStart = paths.length > 0 && !isRunning && !isStarting;
  const isBasicMode = modelsStatus && !modelsStatus.canRunFull;

  const handleStart = async () => {
    if (!canStart) return;
    setIsStarting(true);
    try {
      const params: SmartCullingStartParams = {
        paths,
        mode,
        preset,
        aestheticPreference,
        faceChecks: faceAnalysisEnabled ? faceChecks : [],
        includeEdited,
        previewOnly,
        keepPerGroup,
        faceAnalysisEnabled,
        allowDegraded: true,
      };
      const response = await invoke<{ taskId: string }>(Invokes.SmartCullingStartTask, { params });
      setSmartCulling({
        activeTaskId: response.taskId,
        isRunning: true,
        progress: { taskId: response.taskId, current: 0, total: paths.length, stage: '准备任务' },
        error: null,
        result: null,
        dialogOpen: false,
      });
    } catch (error) {
      setSmartCulling({ error: String(error) });
    } finally {
      setIsStarting(false);
    }
  };

  const handleCancelTask = async () => {
    if (!activeTaskId || isCancellingTask) return;
    setIsCancellingTask(true);
    try {
      await invoke(Invokes.SmartCullingCancelTask, { taskId: activeTaskId });
      setSmartCulling((state) => ({
        progress: state.progress ? { ...state.progress, stage: '正在取消任务' } : state.progress,
        error: null,
      }));
    } catch (error) {
      setSmartCulling({ error: String(error) });
      setIsCancellingTask(false);
    }
  };

  const handleOpenModelsDir = () => {
    invoke<string>(Invokes.SmartCullingOpenModelsDir)
      .then((path) => open(path))
      .catch((error) => setSmartCulling({ error: String(error) }));
  };

  const refreshModelsStatus = async () => {
    try {
      setModelsStatus(await invoke<SmartCullingModelsStatus>(Invokes.SmartCullingCheckModels));
    } catch (error) {
      setModelsStatus({
        modelsDir: '',
        manifestFound: false,
        canRunFull: false,
        canRunBasic: true,
        degradedReason: String(error),
        missingRequired: [],
        missingOptional: [],
      });
    }
  };

  const handleDownloadModels = async () => {
    if (isDownloadingModels) return;
    setIsDownloadingModels(true);
    try {
      const status = await invoke<SmartCullingModelsStatus>(Invokes.SmartCullingDownloadModels);
      setModelsStatus(status);
    } catch (error) {
      setSmartCulling({ error: String(error) });
      await refreshModelsStatus();
    } finally {
      setIsDownloadingModels(false);
    }
  };

  const refreshRecentTasks = async () => {
    try {
      setRecentTasks(await invoke<SmartCullingHistoryItem[]>(Invokes.SmartCullingListRecentTasks));
    } catch {
      setRecentTasks([]);
    }
  };

  const refreshUserPresets = async () => {
    try {
      setUserPresets(await invoke<SmartCullingUserPreset[]>(Invokes.SmartCullingListPresets));
    } catch {
      setUserPresets([]);
    }
  };

  const applyPresetConfig = (config: SmartCullingPresetConfig) => {
    setMode(config.mode);
    setPreset(config.preset);
    setAestheticPreference(config.aestheticPreference);
    setIncludeEdited(config.includeEdited);
    setPreviewOnly(config.previewOnly);
    setKeepPerGroup(Math.min(5, Math.max(1, config.keepPerGroup || 1)));
    setFaceAnalysisEnabled(config.faceAnalysisEnabled);
    setFaceChecks(config.faceChecks.length > 0 ? config.faceChecks : faceChecks);
  };

  const handleUserPresetSelect = (presetId: string) => {
    setSelectedUserPresetId(presetId);
    if (!presetId) {
      setPresetName('');
      return;
    }
    const userPreset = userPresets.find((item) => item.id === presetId);
    if (!userPreset) return;
    setPresetName(userPreset.name);
    applyPresetConfig(userPreset.config);
  };

  const handleSaveUserPreset = async () => {
    if (isSavingPreset) return;
    setIsSavingPreset(true);
    try {
      const params: SmartCullingSavePresetParams = {
        id: selectedUserPresetId || null,
        name: presetName.trim() || defaultPresetName,
        config: currentPresetConfig,
      };
      const savedPreset = await invoke<SmartCullingUserPreset>(Invokes.SmartCullingSavePreset, { params });
      setSelectedUserPresetId(savedPreset.id);
      setPresetName(savedPreset.name);
      await refreshUserPresets();
    } catch (error) {
      setSmartCulling({ error: String(error) });
    } finally {
      setIsSavingPreset(false);
    }
  };

  const handleDeleteUserPreset = async () => {
    if (!selectedUserPresetId || isDeletingPreset) return;
    setIsDeletingPreset(true);
    try {
      await invoke(Invokes.SmartCullingDeletePreset, { id: selectedUserPresetId });
      setSelectedUserPresetId('');
      setPresetName('');
      await refreshUserPresets();
    } catch (error) {
      setSmartCulling({ error: String(error) });
    } finally {
      setIsDeletingPreset(false);
    }
  };

  const toggleFaceCheck = (check: SmartCullingFaceCheck) => {
    setFaceChecks((current) =>
      current.includes(check) ? current.filter((item) => item !== check) : [...current, check],
    );
  };

  const handleOpenRecentTask = async (taskId: string) => {
    try {
      const taskResult = await invoke<SmartCullingTaskResult>(Invokes.SmartCullingGetTaskResult, { taskId });
      setSmartCulling({
        activeTaskId: taskId,
        result: taskResult,
        progress: null,
        isRunning: false,
        error: null,
        dialogOpen: false,
      });
      setUI({ activeView: SMART_CULLING_REVIEW_VIEW });
    } catch (error) {
      setSmartCulling({ error: String(error) });
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
      <div className="w-[720px] max-w-[calc(100vw-32px)] max-h-[calc(100vh-32px)] overflow-y-auto bg-bg-secondary border border-border-color rounded-lg shadow-2xl">
        <div className="p-5 border-b border-border-color flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Sparkles className="text-accent" size={22} />
            <div>
              <Text variant={TextVariants.title} weight={TextWeights.semibold}>
                智能选图
              </Text>
              <Text variant={TextVariants.small} color={TextColors.secondary}>
                {currentFolderPath || '当前图库'} · {paths.length} 张待分析
              </Text>
            </div>
          </div>
          <button
            type="button"
            className="text-text-secondary hover:text-text-primary"
            onClick={() => setSmartCulling({ dialogOpen: false })}
          >
            关闭
          </button>
        </div>

        <div className="p-5 space-y-5">
          {isBasicMode && (
            <div className="flex items-start gap-3 rounded-md border border-yellow-500/40 bg-yellow-500/10 p-3">
              <TriangleAlert size={18} className="text-yellow-500 mt-0.5" />
              <div>
                <Text variant={TextVariants.label} weight={TextWeights.semibold}>
                  当前将以基础模式运行
                </Text>
                <Text variant={TextVariants.small} color={TextColors.secondary}>
                  未检测到 CLIP 模型，系统会使用本地 RAW
                  分析、清晰度、曝光和相似度规则生成结果，并标记可信度降低。下载模型不会阻塞当前基础分析。
                </Text>
              </div>
            </div>
          )}

          <div className="grid grid-cols-2 gap-4">
            <div>
              <Text as="div" variant={TextVariants.label} className="mb-2">
                分析范围
              </Text>
              <Dropdown
                value={range}
                onChange={(value) => setRange(value)}
                options={SMART_CULLING_RANGES.map((item) => ({ value: item.value, label: item.label }))}
              />
            </div>
            <div>
              <Text as="div" variant={TextVariants.label} className="mb-2">
                选图模式
              </Text>
              <Dropdown
                value={mode}
                onChange={(value) => setMode(value)}
                options={SMART_CULLING_MODES.map((item) => ({ value: item.value, label: item.label }))}
              />
            </div>
            <div>
              <Text as="div" variant={TextVariants.label} className="mb-2">
                筛选预设
              </Text>
              <Dropdown
                value={preset}
                onChange={(value) => setPreset(value)}
                options={SMART_CULLING_PRESETS.map((item) => ({ value: item.value, label: item.label }))}
              />
            </div>
            <div>
              <Text as="div" variant={TextVariants.label} className="mb-2">
                美学偏好
              </Text>
              <Dropdown
                value={aestheticPreference}
                onChange={(value) => setAestheticPreference(value)}
                options={SMART_CULLING_AESTHETIC_PREFERENCES.map((item) => ({ value: item.value, label: item.label }))}
              />
            </div>
            <div>
              <Text as="div" variant={TextVariants.label} className="mb-2">
                每组保留
              </Text>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  className="w-9 h-9 rounded-md bg-surface hover:bg-surface-hover"
                  onClick={() => setKeepPerGroup((value) => Math.max(1, value - 1))}
                >
                  -
                </button>
                <div className="h-9 min-w-14 rounded-md bg-surface flex items-center justify-center">
                  <Text variant={TextVariants.label}>{keepPerGroup}</Text>
                </div>
                <button
                  type="button"
                  className="w-9 h-9 rounded-md bg-surface hover:bg-surface-hover"
                  onClick={() => setKeepPerGroup((value) => Math.min(5, value + 1))}
                >
                  +
                </button>
              </div>
            </div>
          </div>

          <div className="rounded-md bg-surface p-3">
            <div className="flex items-center justify-between gap-3">
              <Text variant={TextVariants.label} weight={TextWeights.semibold}>
                配置预设
              </Text>
              <Text variant={TextVariants.small} color={TextColors.secondary}>
                仅保存策略配置，不保存本次分析范围
              </Text>
            </div>
            <div className="mt-3 grid grid-cols-[1fr_1fr_auto_auto] gap-3 items-center">
              <Dropdown
                value={selectedUserPresetId || 'none'}
                onChange={(value) => handleUserPresetSelect(value === 'none' ? '' : value)}
                options={[
                  { value: 'none', label: '选择用户预设' },
                  ...userPresets.map((item) => ({ value: item.id, label: item.name })),
                ]}
              />
              <input
                className="h-10 rounded-md bg-bg-primary px-3 text-text-primary outline-hidden border border-border-color focus:border-accent"
                value={presetName}
                onChange={(event) => setPresetName(event.target.value)}
                placeholder={defaultPresetName}
              />
              <Button
                className="bg-bg-primary text-text-primary"
                onClick={handleSaveUserPreset}
                disabled={isSavingPreset}
              >
                {isSavingPreset ? <Loader2 size={16} className="animate-spin" /> : <Save size={16} />}
                保存
              </Button>
              <Button
                className="bg-bg-primary text-text-primary"
                onClick={handleDeleteUserPreset}
                disabled={!selectedUserPresetId || isDeletingPreset}
              >
                {isDeletingPreset ? <Loader2 size={16} className="animate-spin" /> : <Trash2 size={16} />}
                删除
              </Button>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <Switch label="包含已修图照片" checked={includeEdited} onChange={setIncludeEdited} />
            <Switch label="仅分析不写入" checked={previewOnly} onChange={setPreviewOnly} />
            <Switch label="启用人像表情分析" checked={faceAnalysisEnabled} onChange={setFaceAnalysisEnabled} />
          </div>

          {faceAnalysisEnabled && (
            <div className="rounded-md bg-surface p-3">
              <Text variant={TextVariants.label} weight={TextWeights.semibold}>
                人像判定项
              </Text>
              <div className="mt-3 grid grid-cols-2 gap-3">
                {SMART_CULLING_FACE_CHECKS.map((check) => (
                  <Switch
                    key={check.value}
                    label={check.label}
                    checked={faceChecks.includes(check.value)}
                    onChange={() => toggleFaceCheck(check.value)}
                  />
                ))}
              </div>
            </div>
          )}

          {range === 'selected' && selectedPaths.length === 0 && (
            <Text color={TextColors.error}>当前没有选中图片，请切换分析范围或先选择图片。</Text>
          )}

          {isRunning && progress && (
            <div className="rounded-md bg-surface p-3">
              <Text variant={TextVariants.label}>{progress.stage}</Text>
              <div className="mt-2 h-2 rounded-full bg-bg-primary overflow-hidden">
                <div
                  className="h-full bg-accent"
                  style={{ width: `${progress.total > 0 ? (progress.current / progress.total) * 100 : 0}%` }}
                />
              </div>
            </div>
          )}

          {recentTasks.length > 0 && (
            <div className="rounded-md bg-surface p-3">
              <Text variant={TextVariants.label} weight={TextWeights.semibold}>
                最近任务
              </Text>
              <div className="mt-2 space-y-2">
                {recentTasks.map((task) => (
                  <button
                    key={task.taskId}
                    type="button"
                    className="w-full flex items-center justify-between gap-3 rounded-md bg-bg-primary px-3 py-2 text-left hover:bg-surface-hover"
                    onClick={() => handleOpenRecentTask(task.taskId)}
                  >
                    <Text variant={TextVariants.small} className="truncate">
                      {new Date(task.createdAt).toLocaleString()} · 精选 {task.summary.selected} · 待确认{' '}
                      {task.summary.review} · 淘汰 {task.summary.rejectSuggestion}
                    </Text>
                    <Text variant={TextVariants.small} color={TextColors.secondary}>
                      {task.status}
                    </Text>
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>

        <div className="p-5 border-t border-border-color flex justify-between items-center">
          <div className="flex gap-2">
            <Button className="bg-surface text-text-primary" onClick={handleOpenModelsDir}>
              <FolderOpen size={16} />
              模型目录
            </Button>
            <Button
              className="bg-surface text-text-primary"
              onClick={handleDownloadModels}
              disabled={isDownloadingModels || modelsStatus?.canRunFull}
            >
              {isDownloadingModels ? <Loader2 size={16} className="animate-spin" /> : <Download size={16} />}
              {modelsStatus?.canRunFull ? '模型已就绪' : '下载模型'}
            </Button>
          </div>
          <div className="flex gap-3">
            <Button className="bg-surface text-text-primary" onClick={() => setSmartCulling({ dialogOpen: false })}>
              取消
            </Button>
            {isRunning ? (
              <Button onClick={handleCancelTask} disabled={!activeTaskId || isCancellingTask}>
                {isCancellingTask ? <Loader2 size={16} className="animate-spin" /> : <Square size={16} />}
                取消任务
              </Button>
            ) : (
              <Button onClick={handleStart} disabled={!canStart}>
                {isStarting ? <Loader2 size={16} className="animate-spin" /> : <Sparkles size={16} />}
                开始分析
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
