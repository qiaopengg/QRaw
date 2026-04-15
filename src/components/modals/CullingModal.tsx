import { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { CheckCircle, XCircle, Loader2, Users, Trash2, Star, Tag, Sparkles } from 'lucide-react';
import { AnimatePresence, motion } from 'framer-motion';
import { CullingSettingsV4, Invokes, Progress, SceneTypeV4, CullingSuggestions } from '../ui/AppProperties';
import Button from '../ui/Button';
import Switch from '../ui/Switch';
import Slider from '../ui/Slider';
import Dropdown from '../ui/Dropdown';
import Text from '../ui/Text';
import { TextColors, TextVariants } from '../../types/typography';

interface CullingModalProps {
  isOpen: boolean;
  onClose(): void;
  progress: Progress | null;
  suggestions: CullingSuggestions | null;
  error: string | null;
  imagePaths: string[];
  thumbnails: Record<string, string>;
  onApply(action: 'reject' | 'rate_zero' | 'delete', paths: string[]): void;
  onError(error: string): void;
}

type CullAction = 'reject' | 'rate_zero' | 'delete';

function getCullActions(t: (key: string) => string): {
  value: CullAction;
  label: string;
  icon: React.ReactNode;
}[] {
  return [
    { value: 'reject', label: t('culling.markAsRejected'), icon: <Tag size={16} className="text-red-500" /> },
    { value: 'rate_zero', label: t('culling.setRatingToOne'), icon: <Star size={16} /> },
    { value: 'delete', label: t('culling.moveToTrash'), icon: <Trash2 size={16} /> },
  ];
}

interface ImageThumbnailProps {
  path: string;
  thumbnails: Record<string, string>;
  isSelected: boolean;
  onToggle: () => void;
  children?: React.ReactNode;
}

function ImageThumbnail({ path, thumbnails, isSelected, onToggle, children }: ImageThumbnailProps) {
  const thumbnailUrl = thumbnails[path];
  return (
    <div
      className={`relative group rounded-md overflow-hidden border-2 transition-colors cursor-pointer aspect-[4/3] ${
        isSelected ? 'border-accent' : 'border-transparent hover:border-surface'
      }`}
      onClick={onToggle}
    >
      <img
        src={thumbnailUrl}
        alt={path}
        className={`absolute inset-0 w-full h-full object-cover transition-opacity ${isSelected ? 'opacity-100' : 'opacity-75 group-hover:opacity-100'}`}
      />
      <div
        className={`absolute inset-0 bg-black/50 transition-opacity ${
          isSelected ? 'opacity-0' : 'opacity-100 group-hover:opacity-0'
        }`}
      />
      <div className="absolute top-2 right-2 z-10">
        {isSelected && <CheckCircle size={16} className="text-accent" />}
      </div>
      {children && (
        <div className="absolute bottom-0 left-0 right-0 p-1.5 bg-black/60 flex flex-col gap-0.5 z-10">{children}</div>
      )}
    </div>
  );
}

export default function CullingModal({
  isOpen,
  onClose,
  progress,
  suggestions,
  error,
  imagePaths,
  thumbnails,
  onApply,
  onError,
}: CullingModalProps) {
  const [isMounted, setIsMounted] = useState(false);
  const [show, setShow] = useState(false);
  const [stage, setStage] = useState<'settings' | 'progress' | 'results'>('settings');
  const { t } = useTranslation();
  const [isStarting, setIsStarting] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [preset, setPreset] = useState<'balanced' | 'conservative' | 'aggressive'>('balanced');

  const [settings, setSettings] = useState<CullingSettingsV4>({
    blurThreshold: 100.0,
    similarityThreshold: 28,
    earThreshold: 0.2,
    enableNimaAesthetic: true,
    enableAutoScene: true,
    manualProfile: 'default',
    strictness: 'balanced',
  });

  const [selectedRejects, setSelectedRejects] = useState<Set<string>>(new Set());
  const [action, setAction] = useState<CullAction>('reject');
  const [activeTab, setActiveTab] = useState<'similar' | 'blurry' | 'badExpressions'>('similar');

  const actionOptions = useMemo(() => getCullActions(t), [t]);

  const formatReasons = useCallback(
    (reasons?: string[]) => {
      if (!reasons || reasons.length === 0) return '';
      return reasons.map((r) => t(`culling.reasons.${r}`, { defaultValue: r })).join(', ');
    },
    [t],
  );

  useEffect(() => {
    if (isOpen) {
      setIsMounted(true);
      const timer = setTimeout(() => setShow(true), 10);
      return () => clearTimeout(timer);
    } else {
      setShow(false);
      const timer = setTimeout(() => {
        setIsMounted(false);
        setStage('settings');
        setSelectedRejects(new Set());
        setIsStarting(false);
      }, 300);
      return () => clearTimeout(timer);
    }
  }, [isOpen]);

  useEffect(() => {
    if (suggestions || error) {
      setStage('results');
    } else if (progress) {
      setStage('progress');
    } else if (isOpen) {
      setStage('settings');
    }
  }, [progress, suggestions, error, isOpen]);

  useEffect(() => {
    if (stage === 'results' && suggestions) {
      setSelectedRejects(new Set());
    }
  }, [stage, suggestions]);

  useEffect(() => {
    if (!isOpen) return;
    setSettings((s) => {
      if (preset === 'balanced') {
        return {
          ...s,
          blurThreshold: 100.0,
          similarityThreshold: 28,
          earThreshold: 0.2,
          strictness: 'balanced' as const,
        };
      }
      if (preset === 'conservative') {
        return {
          ...s,
          blurThreshold: 75.0,
          similarityThreshold: 22,
          earThreshold: 0.25,
          strictness: 'conservative' as const,
        };
      }
      return {
        ...s,
        blurThreshold: 150.0,
        similarityThreshold: 36,
        earThreshold: 0.18,
        strictness: 'aggressive' as const,
      };
    });
  }, [preset, isOpen]);

  const handleStartCulling = useCallback(async () => {
    if (isStarting) return;
    try {
      setIsStarting(true);
      setStage('progress');
      await invoke(Invokes.CullImagesV4, { paths: imagePaths, settings });
    } catch (err) {
      console.error('Culling failed to start:', err);
      onError(String(err));
    } finally {
      setIsStarting(false);
    }
  }, [imagePaths, settings, onError]);

  const handleToggleReject = (path: string) => {
    setSelectedRejects((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(path)) {
        newSet.delete(path);
      } else {
        newSet.add(path);
      }
      return newSet;
    });
  };

  const handleApply = () => {
    onApply(action, Array.from(selectedRejects));
  };

  const handleSelectAllDuplicates = () => {
    if (!suggestions) return;
    const next = new Set<string>();
    suggestions.similarGroups.forEach((group) => group.duplicates.forEach((d) => next.add(d.path)));
    setSelectedRejects(next);
  };

  const handleSelectAllBlurry = () => {
    if (!suggestions) return;
    const next = new Set<string>();
    suggestions.blurryImages.forEach((img) => next.add(img.path));
    setSelectedRejects(next);
  };

  const handleSelectAllBadExpr = () => {
    if (!suggestions) return;
    const next = new Set<string>();
    suggestions.badExpressions?.forEach((img) => next.add(img.path));
    setSelectedRejects(next);
  };

  const handleClearSelection = () => {
    setSelectedRejects(new Set());
  };

  const numSimilar = suggestions?.similarGroups.reduce((acc, group) => acc + group.duplicates.length, 0) || 0;
  const numBlurry = suggestions?.blurryImages.length || 0;
  const numBadExpr = suggestions?.badExpressions?.length || 0;

  const renderSettings = () => (
    <>
      <div className="flex items-center justify-center mb-4 shrink-0">
        <Users className="w-12 h-12 text-accent" />
      </div>
      <Text variant={TextVariants.title} className="mb-4 text-center shrink-0">
        {t('culling.title') || 'AI 智能选图'}
      </Text>
      <div className="flex-1 overflow-y-auto min-h-0 space-y-6 text-sm pr-1">
        <div>
          <div className="mb-1">
            <Text variant={TextVariants.label}>{'拍摄场景'}</Text>
          </div>
          <Dropdown
            value={settings.manualProfile}
            onChange={(v: string) => setSettings((s) => ({ ...s, manualProfile: v as any }))}
            options={[
              { value: 'default', label: '默认（自动识别）' },
              { value: 'closeUpPortrait', label: '人像' },
              { value: 'landscape', label: '风光' },
              { value: 'groupPhoto', label: '合影 / 活动' },
              { value: 'wedding', label: '婚礼' },
            ]}
          />
          <Text variant={TextVariants.small} color={TextColors.secondary} className="mt-1 mb-4 block">
            {'根据拍摄类型自动调整评分权重。选择"默认"时系统会自动识别场景。'}
          </Text>
        </div>
        <div>
          <div className="mb-1">
            <Text variant={TextVariants.label}>{'筛选力度'}</Text>
          </div>
          <Dropdown
            value={preset}
            onChange={(v: string) => setPreset(v as any)}
            options={[
              { value: 'balanced', label: '均衡' },
              { value: 'conservative', label: '保守（保留更多照片）' },
              { value: 'aggressive', label: '激进（严格筛选）' },
            ]}
          />
          <Text variant={TextVariants.small} color={TextColors.secondary} className="mt-1 block">
            {'控制整体筛选的严格程度。保守模式会保留更多照片，激进模式会更严格地淘汰。'}
          </Text>
        </div>
        <div className="flex items-center justify-between">
          <Text variant={TextVariants.label}>{'高级选项'}</Text>
          <button
            className="px-3 py-1 rounded-md text-text-secondary hover:bg-surface transition-colors"
            onClick={() => setShowAdvanced((v) => !v)}
            type="button"
          >
            {showAdvanced ? '收起' : '展开'}
          </button>
        </div>
        {showAdvanced && (
          <div className="space-y-5 pl-2 border-l-2 border-border-color ml-1">
            <div>
              <Slider
                label={'相似度阈值'}
                min={1}
                max={64}
                step={1}
                value={settings.similarityThreshold}
                defaultValue={28}
                onChange={(e) => setSettings((s) => ({ ...s, similarityThreshold: Number(e.target.value) }))}
              />
              <Text variant={TextVariants.small} color={TextColors.secondary} className="mt-1">
                {'值越低，连拍分组要求越严格（需要更相似才归为一组）。'}
              </Text>
            </div>
            <div>
              <Slider
                label={'模糊阈值'}
                min={25}
                max={500}
                step={25}
                value={settings.blurThreshold}
                defaultValue={100.0}
                onChange={(e) => setSettings((s) => ({ ...s, blurThreshold: Number(e.target.value) }))}
              />
              <Text variant={TextVariants.small} color={TextColors.secondary} className="mt-1">
                {'值越高，对清晰度要求越严格。低于此阈值的照片会被标记为模糊。'}
              </Text>
            </div>
            <div>
              <Slider
                label={'闭眼检测灵敏度'}
                min={0.1}
                max={0.35}
                step={0.01}
                value={settings.earThreshold}
                defaultValue={0.2}
                onChange={(e) => setSettings((s) => ({ ...s, earThreshold: Number(e.target.value) }))}
              />
              <Text variant={TextVariants.small} color={TextColors.secondary} className="mt-1">
                {'值越低，闭眼检测越严格（更容易判定为闭眼）。需要 2d106det 模型支持。'}
              </Text>
            </div>
            <div>
              <Switch
                label={'自动场景识别'}
                checked={settings.enableAutoScene}
                onChange={(v) => setSettings((s) => ({ ...s, enableAutoScene: v }))}
              />
              <Text variant={TextVariants.small} color={TextColors.secondary} className="mt-1 ml-12">
                {'开启后系统会根据照片内容自动判断拍摄场景，关闭则使用上方手动选择的场景。'}
              </Text>
            </div>
            <div>
              <Switch
                label={'美学评分'}
                checked={settings.enableNimaAesthetic}
                onChange={(v) => setSettings((s) => ({ ...s, enableNimaAesthetic: v }))}
              />
              <Text variant={TextVariants.small} color={TextColors.secondary} className="mt-1 ml-12">
                {'开启后使用 AI 模型评估照片的美学质量（构图、色彩等）。需要 NIMA 模型支持。'}
              </Text>
            </div>
          </div>
        )}
      </div>
      <div className="flex justify-end gap-3 mt-6 pt-4 border-t border-surface shrink-0">
        <button
          className="px-4 py-2 rounded-md text-text-secondary hover:bg-surface transition-colors"
          onClick={onClose}
        >
          {t('common.cancel') || '取消'}
        </button>
        <Button onClick={handleStartCulling} disabled={isStarting || imagePaths.length === 0}>
          {t('culling.startCulling') || '开始智能选图'}
        </Button>
      </div>
    </>
  );

  const renderProgress = () => (
    <div className="flex flex-col items-center justify-center h-48">
      <Loader2 className="w-16 h-16 text-accent animate-spin" />
      <p className="mt-4 text-text-primary">{progress?.stage || t('culling.starting')}</p>
      {progress && progress.total > 0 && (
        <div className="w-full bg-surface rounded-full h-2.5 mt-2">
          <div
            className="bg-accent h-2.5 rounded-full"
            style={{ width: `${((progress.current ?? 0) / progress.total) * 100}%` }}
          />
        </div>
      )}
    </div>
  );

  const renderResults = () => {
    if (error) {
      return (
        <div className="flex flex-col items-center justify-center h-48">
          <XCircle className="w-16 h-16 text-red-500" />
          <Text variant={TextVariants.heading} className="mt-4 text-center">
            {t('culling.cullingFailed')}
          </Text>
          <Text>{error}</Text>
          <div className="mt-6">
            <Button onClick={onClose}>{t('common.close')}</Button>
          </div>
        </div>
      );
    }

    if (!suggestions) return null;

    const totalSuggestions = numSimilar + numBlurry;
    if (totalSuggestions === 0) {
      return (
        <div className="flex flex-col items-center justify-center h-48">
          <CheckCircle className="w-16 h-16 text-green-500" />
          <Text variant={TextVariants.heading} className="mt-4">
            {t('culling.noIssuesFound')}
          </Text>
          <Text>{t('culling.allImagesGood')}</Text>
          <div className="mt-6">
            <Button onClick={onClose}>{t('common.done')}</Button>
          </div>
        </div>
      );
    }

    return (
      <>
        <div className="flex items-start justify-between gap-4 mb-4 shrink-0">
          <div>
            <Text variant={TextVariants.title}>{t('culling.cullingSuggestions')}</Text>
            <Text variant={TextVariants.small} color={TextColors.secondary} className="mt-1 block">
              {t('culling.autoAppliedNotice')}
            </Text>
          </div>
          <div className="shrink-0 flex items-center gap-2 text-text-secondary">
            <Sparkles size={18} className="text-accent" />
            <Text variant={TextVariants.small} color={TextColors.secondary}>
              {t('culling.autoAppliedNoticeDetail')}
            </Text>
          </div>
        </div>
        <div className="border-b border-surface mb-4 shrink-0 overflow-x-auto">
          <nav className="-mb-px flex space-x-4 min-w-max px-2" aria-label="Tabs">
            {numSimilar > 0 && (
              <button
                onClick={() => setActiveTab('similar')}
                className={`${
                  activeTab === 'similar'
                    ? 'border-accent text-accent'
                    : 'border-transparent text-text-secondary hover:text-text-primary hover:border-gray-300'
                } whitespace-nowrap py-2 px-1 border-b-2 font-medium text-sm`}
              >
                {t('culling.similarGroups')}{' '}
                <span className="bg-surface text-text-secondary rounded-full px-2 py-0.5 text-xs">{numSimilar}</span>
              </button>
            )}
            {numBlurry > 0 && (
              <button
                onClick={() => setActiveTab('blurry')}
                className={`${
                  activeTab === 'blurry'
                    ? 'border-accent text-accent'
                    : 'border-transparent text-text-secondary hover:text-text-primary hover:border-gray-300'
                } whitespace-nowrap py-2 px-1 border-b-2 font-medium text-sm`}
              >
                {t('culling.blurryImages')}{' '}
                <span className="bg-surface text-text-secondary rounded-full px-2 py-0.5 text-xs">{numBlurry}</span>
              </button>
            )}
            {numBadExpr > 0 && (
              <button
                onClick={() => setActiveTab('badExpressions')}
                className={`${
                  activeTab === 'badExpressions'
                    ? 'border-accent text-accent'
                    : 'border-transparent text-text-secondary hover:text-text-primary hover:border-gray-300'
                } whitespace-nowrap py-2 px-1 border-b-2 font-medium text-sm`}
              >
                {t('culling.badExpressions') || 'Bad Expressions'}{' '}
                <span className="bg-surface text-text-secondary rounded-full px-2 py-0.5 text-xs">{numBadExpr}</span>
              </button>
            )}
          </nav>
        </div>

        <div className="bg-bg-primary rounded-lg p-4 flex-1 overflow-y-auto min-h-0">
          <div className="flex flex-wrap items-center justify-between gap-2 mb-3">
            <div className="flex flex-wrap items-center gap-2">
              {activeTab === 'similar' && numSimilar > 0 && (
                <button
                  type="button"
                  className="px-3 py-1 rounded-md text-text-secondary hover:bg-surface transition-colors"
                  onClick={handleSelectAllDuplicates}
                >
                  {t('culling.selectAllDuplicates')}
                </button>
              )}
              {activeTab === 'blurry' && numBlurry > 0 && (
                <button
                  type="button"
                  className="px-3 py-1 rounded-md text-text-secondary hover:bg-surface transition-colors"
                  onClick={handleSelectAllBlurry}
                >
                  {t('culling.selectAllBlurry')}
                </button>
              )}
              {activeTab === 'badExpressions' && numBadExpr > 0 && (
                <button
                  type="button"
                  className="px-3 py-1 rounded-md text-text-secondary hover:bg-surface transition-colors"
                  onClick={handleSelectAllBadExpr}
                >
                  {t('culling.selectAllBadExpr') || 'Select All'}
                </button>
              )}
              <button
                type="button"
                className="px-3 py-1 rounded-md text-text-secondary hover:bg-surface transition-colors"
                onClick={handleClearSelection}
              >
                {t('common.selectNone')}
              </button>
            </div>
            <Text variant={TextVariants.small} color={TextColors.secondary}>
              {t('culling.selectedCount', { count: selectedRejects.size })}
            </Text>
          </div>
          <AnimatePresence mode="wait">
            <motion.div
              key={activeTab}
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -10 }}
              transition={{ duration: 0.2 }}
            >
              {activeTab === 'similar' && (
                <div className="space-y-4">
                  {suggestions.similarGroups.map((group, index) => (
                    <div key={index} className="bg-surface rounded-lg p-3">
                      <Text variant={TextVariants.heading} className="mb-2">
                        {t('culling.group', { index: index + 1 })}
                      </Text>
                      <div className="flex flex-col md:flex-row gap-4">
                        <div className="w-full md:w-1/3 lg:w-1/4 shrink-0">
                          <Text variant={TextVariants.label} className="mb-1">
                            {t('culling.bestImage')}
                          </Text>
                          <div className="relative rounded-md overflow-hidden border-2 border-green-500 aspect-[4/3]">
                            <img
                              src={thumbnails[group.representative.path]}
                              alt="Representative"
                              className="absolute inset-0 w-full h-full object-cover"
                            />
                            <div className="absolute bottom-0 left-0 right-0 p-1.5 bg-black/60 flex flex-col gap-0.5 z-10">
                              <Text as="div" variant={TextVariants.small} color={TextColors.white}>
                                {t('culling.score', { score: group.representative.qualityScore.toFixed(2) })}
                              </Text>
                              {group.representative.reasons?.length > 0 && (
                                <Text as="div" variant={TextVariants.small} color={TextColors.accent}>
                                  {formatReasons(group.representative.reasons)}
                                </Text>
                              )}
                              {group.representative.faceDetectorType && (
                                <Text as="div" variant={TextVariants.small} color={TextColors.secondary}>
                                  Face: {group.representative.faceDetectorType}
                                </Text>
                              )}
                            </div>
                          </div>
                        </div>
                        <div className="flex-1">
                          <Text variant={TextVariants.label} className="mb-1">
                            {t('culling.duplicates', { count: group.duplicates.length })}
                          </Text>
                          <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-2">
                            {group.duplicates.map((dup) => (
                              <ImageThumbnail
                                key={dup.path}
                                path={dup.path}
                                thumbnails={thumbnails}
                                isSelected={selectedRejects.has(dup.path)}
                                onToggle={() => handleToggleReject(dup.path)}
                              >
                                <Text as="div" variant={TextVariants.small} color={TextColors.white}>
                                  {t('culling.score', { score: dup.qualityScore.toFixed(2) })}
                                </Text>
                                {dup.reasons?.length > 0 && (
                                  <Text as="div" variant={TextVariants.small} color={TextColors.accent}>
                                    {formatReasons(dup.reasons)}
                                  </Text>
                                )}
                                {dup.faceDetectorType && (
                                  <Text as="div" variant={TextVariants.small} color={TextColors.secondary}>
                                    Face: {dup.faceDetectorType}
                                  </Text>
                                )}
                              </ImageThumbnail>
                            ))}
                          </div>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
              {activeTab === 'blurry' && (
                <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-3">
                  {suggestions.blurryImages.map((img) => (
                    <ImageThumbnail
                      key={img.path}
                      path={img.path}
                      thumbnails={thumbnails}
                      isSelected={selectedRejects.has(img.path)}
                      onToggle={() => handleToggleReject(img.path)}
                    >
                      <Text as="div" variant={TextVariants.small} color={TextColors.white}>
                        {t('culling.sharpnessScore', { score: img.sharpnessMetric.toFixed(0) })}
                      </Text>
                      {img.reasons?.length > 0 && (
                        <Text as="div" variant={TextVariants.small} color={TextColors.accent}>
                          {formatReasons(img.reasons)}
                        </Text>
                      )}
                      {img.faceDetectorType && (
                        <Text as="div" variant={TextVariants.small} color={TextColors.secondary}>
                          Face: {img.faceDetectorType}
                        </Text>
                      )}
                    </ImageThumbnail>
                  ))}
                </div>
              )}
              {activeTab === 'badExpressions' && (
                <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-3">
                  {suggestions.badExpressions?.map((img) => (
                    <ImageThumbnail
                      key={img.path}
                      path={img.path}
                      thumbnails={thumbnails}
                      isSelected={selectedRejects.has(img.path)}
                      onToggle={() => handleToggleReject(img.path)}
                    >
                      <Text as="div" variant={TextVariants.small} color={TextColors.white}>
                        {t('culling.score', { score: img.qualityScore.toFixed(2) })}
                      </Text>
                      {img.reasons?.length > 0 && (
                        <Text as="div" variant={TextVariants.small} color={TextColors.accent}>
                          {formatReasons(img.reasons)}
                        </Text>
                      )}
                      {img.faceDetectorType && (
                        <Text as="div" variant={TextVariants.small} color={TextColors.secondary}>
                          Face: {img.faceDetectorType}
                        </Text>
                      )}
                    </ImageThumbnail>
                  ))}
                </div>
              )}
            </motion.div>
          </AnimatePresence>
        </div>

        <div className="flex justify-between items-center gap-3 mt-6 pt-4 border-t border-surface shrink-0">
          <div className="flex-1">
            <Dropdown
              options={actionOptions.map(({ value, label }) => ({ value, label }))}
              value={action}
              onChange={(newValue: CullAction) => setAction(newValue)}
              className="w-full"
            />
          </div>
          <div className="flex gap-3">
            <button
              className="px-4 py-2 rounded-md text-text-secondary hover:bg-surface transition-colors"
              onClick={onClose}
            >
              {t('common.cancel')}
            </button>
            <Button
              onClick={() => {
                if (action === 'delete') {
                  const ok = window.confirm(t('culling.confirmDelete'));
                  if (!ok) return;
                }
                handleApply();
              }}
              disabled={selectedRejects.size === 0}
            >
              {t('culling.applyToImages', { count: selectedRejects.size })}
            </Button>
          </div>
        </div>
      </>
    );
  };

  const renderContent = () => {
    switch (stage) {
      case 'settings':
        return renderSettings();
      case 'progress':
        return renderProgress();
      case 'results':
        return renderResults();
      default:
        return null;
    }
  };

  if (!isMounted) return null;

  return (
    <div
      className={`fixed inset-0 flex items-center justify-center z-50 bg-black/30 backdrop-blur-xs transition-opacity duration-300 ease-in-out ${
        show ? 'opacity-100' : 'opacity-0'
      }`}
      onClick={() => {
        if (stage !== 'progress') onClose();
      }}
      role="dialog"
      aria-modal="true"
    >
      <div
        className={`bg-surface rounded-lg shadow-xl p-6 w-full max-w-4xl transform transition-all duration-300 ease-out flex flex-col max-h-[90vh] ${
          show ? 'scale-100 opacity-100 translate-y-0' : 'scale-95 opacity-0 -translate-y-4'
        }`}
        onClick={(e) => e.stopPropagation()}
      >
        {renderContent()}
      </div>
    </div>
  );
}
