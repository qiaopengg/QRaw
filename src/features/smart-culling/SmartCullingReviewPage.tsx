import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-shell';
import {
  ArrowLeft,
  Check,
  ChevronDown,
  ChevronRight,
  FileText,
  Loader2,
  RotateCcw,
  Sparkles,
  Star,
  X,
} from 'lucide-react';
import Button from '../../components/ui/Button';
import Dropdown from '../../components/ui/Dropdown';
import Text from '../../components/ui/Text';
import { Invokes } from '../../components/ui/AppProperties';
import { useLibraryStore } from '../../store/useLibraryStore';
import { useProcessStore } from '../../store/useProcessStore';
import { TextColors, TextVariants, TextWeights } from '../../types/typography';
import { COLOR_LABELS } from '../../utils/adjustments';
import type { LibraryFeatureViewSlotProps } from '../contracts';
import { useSmartCullingEvents } from './useSmartCullingEvents';
import { useSmartCullingStore } from './useSmartCulling';
import type {
  SmartCullingApplyResult,
  SmartCullingColorLabel,
  SmartCullingReportResult,
  SmartCullingReviewItem,
  SmartCullingStatus,
  SmartCullingTaskResult,
  SmartCullingUndoResult,
} from './types';

const STATUS_OPTIONS: { value: SmartCullingStatus; label: string }[] = [
  { value: 'selected', label: '精选' },
  { value: 'review', label: '待确认' },
  { value: 'reject_suggestion', label: '淘汰建议' },
];

const COLOR_LABEL_TEXT: Record<SmartCullingColorLabel, string> = {
  red: '红色',
  yellow: '黄色',
  green: '绿色',
  blue: '蓝色',
  purple: '紫色',
  none: '无颜色',
};

const COLOR_OPTIONS: { value: SmartCullingColorLabel | 'keep'; label: string }[] = [
  { value: 'keep', label: '保留颜色' },
  { value: 'none', label: '无颜色' },
  ...COLOR_LABELS.map((color) => ({
    value: color.name as SmartCullingColorLabel,
    label: COLOR_LABEL_TEXT[color.name as SmartCullingColorLabel],
  })),
];

function statusLabel(status: SmartCullingStatus) {
  return STATUS_OPTIONS.find((item) => item.value === status)?.label || '跳过';
}

function statusClass(status: SmartCullingStatus) {
  if (status === 'selected') return 'border-green-500/40 bg-green-500/10';
  if (status === 'reject_suggestion') return 'border-red-500/40 bg-red-500/10';
  if (status === 'failed') return 'border-red-500/40 bg-red-500/10';
  if (status === 'skipped') return 'border-yellow-500/40 bg-yellow-500/10';
  return 'border-border-color bg-surface/40';
}

function statusFromRating(rating: number): SmartCullingStatus {
  if (rating >= 4) return 'selected';
  if (rating >= 2) return 'review';
  return 'reject_suggestion';
}

function ratingForStatus(status: SmartCullingStatus, currentRating: number) {
  if (status === 'selected') return currentRating >= 4 ? currentRating : 4;
  if (status === 'review') return currentRating >= 2 && currentRating <= 3 ? currentRating : 3;
  if (status === 'reject_suggestion') return 1;
  return currentRating;
}

function makeSkippedItem(item: SmartCullingReviewItem, reason: string): SmartCullingReviewItem {
  return {
    ...item,
    rating: 0,
    status: 'skipped',
    colorLabel: null,
    score: 0,
    reasonCodes: ['跳过', '用户复核'],
    reasonText: reason,
    skipReason: reason,
  };
}

function RatingEditor({
  item,
  disabled,
  onChange,
}: {
  item: SmartCullingReviewItem;
  disabled?: boolean;
  onChange(rating: number): void;
}) {
  if (item.status === 'skipped' || item.status === 'failed') return null;
  return (
    <div className="flex items-center gap-1">
      {[1, 2, 3, 4, 5].map((rating) => (
        <button
          key={rating}
          type="button"
          onClick={() => onChange(rating)}
          disabled={disabled}
          className="text-accent disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <Star size={15} className={rating <= item.rating ? 'fill-accent' : ''} />
        </button>
      ))}
    </div>
  );
}

function ReviewCard({
  item,
  readOnly = false,
  onChange,
  onPromoteBest,
}: {
  item: SmartCullingReviewItem;
  readOnly?: boolean;
  onChange(path: string, patch: Partial<SmartCullingReviewItem>): void;
  onPromoteBest?(item: SmartCullingReviewItem): void;
}) {
  const thumb = useProcessStore((state) => state.thumbnails[item.path]);

  return (
    <div className={`rounded-md border p-3 flex gap-3 min-w-0 ${statusClass(item.status)}`}>
      <div className="w-24 h-24 rounded-md bg-bg-primary overflow-hidden shrink-0">
        {thumb ? (
          <img src={thumb} alt={item.fileName} className="w-full h-full object-cover" />
        ) : (
          <div className="w-full h-full flex items-center justify-center">
            <Sparkles size={18} className="text-text-secondary" />
          </div>
        )}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <Text variant={TextVariants.label} weight={TextWeights.semibold} className="truncate">
              {item.fileName}
            </Text>
            <Text variant={TextVariants.small} color={TextColors.secondary} className="truncate">
              {item.reasonText || item.skipReason || '无原因'}
            </Text>
          </div>
          <Text
            as="div"
            variant={TextVariants.small}
            className="shrink-0 rounded-full px-2 py-0.5 bg-bg-primary"
            color={TextColors.secondary}
          >
            {statusLabel(item.status)}
          </Text>
        </div>

        <div className="mt-3 flex items-center gap-3 flex-wrap">
          <RatingEditor item={item} disabled={readOnly} onChange={(rating) => onChange(item.path, { rating })} />
          {item.status !== 'skipped' && item.status !== 'failed' && (
            <div className="w-36">
              <Dropdown
                value={item.status}
                onChange={(status) => onChange(item.path, { status })}
                options={STATUS_OPTIONS}
                triggerClassName="bg-bg-primary"
                disabled={readOnly}
              />
            </div>
          )}
          {item.status !== 'skipped' && item.status !== 'failed' && (
            <div className="w-32">
              <Dropdown
                value={item.colorLabel || 'keep'}
                onChange={(colorLabel) =>
                  onChange(item.path, {
                    colorLabel: colorLabel === 'keep' ? null : (colorLabel as SmartCullingColorLabel),
                  })
                }
                options={COLOR_OPTIONS}
                triggerClassName="bg-bg-primary"
                disabled={readOnly}
              />
            </div>
          )}
          {item.groupId && (
            <Text variant={TextVariants.small} color={TextColors.secondary}>
              相似组 {item.groupRank}/{item.groupSize}
            </Text>
          )}
          {item.groupId && item.groupRank && item.groupRank > 1 && onPromoteBest && !readOnly && (
            <button
              type="button"
              className="text-xs rounded-md bg-bg-primary px-2 py-1 text-text-secondary hover:text-text-primary"
              onClick={() => onPromoteBest(item)}
            >
              设为最优
            </button>
          )}
          {item.degraded && (
            <Text variant={TextVariants.small} color={TextColors.secondary}>
              可信度降低
            </Text>
          )}
        </div>
      </div>
    </div>
  );
}

export default function SmartCullingReviewPage({ onBackToLibrary, onLibraryRefresh }: LibraryFeatureViewSlotProps) {
  useSmartCullingEvents();
  const { activeTaskId, result, error, setSmartCulling } = useSmartCullingStore();
  const [items, setItems] = useState<SmartCullingReviewItem[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isApplying, setIsApplying] = useState(false);
  const [isExportingReport, setIsExportingReport] = useState(false);
  const [isUndoing, setIsUndoing] = useState(false);
  const [reasonFilter, setReasonFilter] = useState<string>('all');
  const [statusFilter, setStatusFilter] = useState<string>('all');
  const [colorFilter, setColorFilter] = useState<string>('all');
  const [ratingFilter, setRatingFilter] = useState<string>('all');
  const [expandedGroups, setExpandedGroups] = useState<Record<string, boolean>>({});
  const setLibrary = useLibraryStore((state) => state.setLibrary);

  useEffect(() => {
    if (result) {
      setItems(result.items);
      return;
    }
    if (!activeTaskId) return;
    setIsLoading(true);
    invoke<SmartCullingTaskResult>(Invokes.SmartCullingGetTaskResult, { taskId: activeTaskId })
      .then((taskResult) => {
        setSmartCulling({ result: taskResult });
        setItems(taskResult.items);
      })
      .finally(() => setIsLoading(false));
  }, [activeTaskId, result, setSmartCulling]);

  const reasonOptions = useMemo(() => {
    const codes = new Set<string>();
    items.forEach((item) => item.reasonCodes.forEach((code) => codes.add(code)));
    return [{ value: 'all', label: '全部原因' }, ...Array.from(codes).map((code) => ({ value: code, label: code }))];
  }, [items]);

  const visibleItems = useMemo(() => {
    return items.filter((item) => {
      if (statusFilter !== 'all' && item.status !== statusFilter) return false;
      if (reasonFilter !== 'all' && !item.reasonCodes.includes(reasonFilter)) return false;
      if (colorFilter !== 'all' && (item.colorLabel || 'keep') !== colorFilter) return false;
      if (ratingFilter !== 'all' && item.rating !== Number(ratingFilter)) return false;
      return true;
    });
  }, [colorFilter, items, ratingFilter, reasonFilter, statusFilter]);

  const groupedItems = useMemo(() => {
    const groups: Record<string, SmartCullingReviewItem[]> = {};
    const singles: SmartCullingReviewItem[] = [];
    visibleItems.forEach((item) => {
      if (!item.groupId) {
        singles.push(item);
        return;
      }
      groups[item.groupId] = groups[item.groupId] || [];
      groups[item.groupId].push(item);
    });
    Object.values(groups).forEach((group) => group.sort((a, b) => (a.groupRank || 999) - (b.groupRank || 999)));
    return { groups: Object.entries(groups), singles };
  }, [visibleItems]);

  const summary = useMemo(() => {
    return {
      analyzed: items.filter((item) => item.status !== 'skipped' && item.status !== 'failed').length,
      skipped: items.filter((item) => item.status === 'skipped').length,
      selected: items.filter((item) => item.status === 'selected').length,
      review: items.filter((item) => item.status === 'review').length,
      rejectSuggestion: items.filter((item) => item.status === 'reject_suggestion').length,
      failed: items.filter((item) => item.status === 'failed').length,
    };
  }, [items]);

  const readOnly = Boolean(result?.previewOnly || result?.status === 'applied' || result?.status === 'revoked');

  const updateItem = (path: string, patch: Partial<SmartCullingReviewItem>) => {
    setItems((prev) =>
      prev.map((item) => {
        if (item.path !== path) return item;
        const normalizedPatch = { ...patch };
        if (typeof patch.rating === 'number' && !patch.status) {
          normalizedPatch.status = statusFromRating(patch.rating);
        }
        if (patch.status && typeof patch.rating !== 'number') {
          normalizedPatch.rating = ratingForStatus(patch.status, item.rating);
        }
        return { ...item, ...normalizedPatch };
      }),
    );
  };

  const promoteGroupBest = (target: SmartCullingReviewItem) => {
    if (!target.groupId) return;
    setItems((prev) => {
      const group = prev
        .filter((item) => item.groupId === target.groupId)
        .sort((a, b) => {
          if (a.path === target.path) return -1;
          if (b.path === target.path) return 1;
          return (a.groupRank || 999) - (b.groupRank || 999);
        });
      const rankByPath = new Map(group.map((item, index) => [item.path, index + 1]));
      return prev.map((item) => {
        const rank = rankByPath.get(item.path);
        if (!rank) return item;
        if (rank === 1) {
          return {
            ...item,
            groupRank: 1,
            status: item.status === 'reject_suggestion' ? 'review' : item.status,
            rating: Math.max(item.rating, 3),
            reasonText: '用户手动设为相似组最优',
          };
        }
        return {
          ...item,
          groupRank: rank,
          rating: Math.min(item.rating, 2),
          status: item.status === 'selected' ? 'review' : item.status,
        };
      });
    });
  };

  const acceptGroupBestOnly = (groupId: string) => {
    setItems((prev) =>
      prev.map((item) => {
        if (item.groupId !== groupId || item.status === 'skipped' || item.status === 'failed') return item;
        if (item.groupRank === 1) {
          return {
            ...item,
            rating: Math.max(item.rating, 4),
            status: 'selected',
            reasonText: item.reasonText || '已按组接受相似组最优',
          };
        }
        return makeSkippedItem(item, '按组接受时仅应用相似组最优');
      }),
    );
  };

  const buildApplyOnlyItems = (status: SmartCullingStatus) => {
    const reason = status === 'selected' ? '只应用精选，本项未写入' : '只应用淘汰建议，本项未写入';
    return items.map((item) => {
      if (item.status === status || item.status === 'skipped' || item.status === 'failed') return item;
      return makeSkippedItem(item, reason);
    });
  };

  const updateImageListAfterItems = (appliedItems: SmartCullingReviewItem[], appliedPaths?: string[]) => {
    setLibrary((state) => {
      const imageRatings = { ...state.imageRatings };
      const appliedPathSet = appliedPaths ? new Set(appliedPaths) : null;
      const byPath = new Map(appliedItems.map((item) => [item.path, item]));
      appliedItems.forEach((item) => {
        if (
          (!appliedPathSet || appliedPathSet.has(item.path)) &&
          item.status !== 'skipped' &&
          item.status !== 'failed'
        ) {
          imageRatings[item.path] = item.rating;
        }
      });
      return {
        imageRatings,
        imageList: state.imageList.map((image) => {
          const item = byPath.get(image.path);
          if (!item || item.status === 'skipped' || item.status === 'failed') return image;
          if (appliedPathSet && !appliedPathSet.has(item.path)) return image;
          const currentTags = image.tags || [];
          const tags =
            item.colorLabel && item.colorLabel !== 'none'
              ? [...currentTags.filter((tag) => !tag.startsWith('color:')), `color:${item.colorLabel}`]
              : item.colorLabel === 'none'
                ? currentTags.filter((tag) => !tag.startsWith('color:'))
                : currentTags;
          return {
            ...image,
            rating: item.rating,
            tags,
            featureData: {
              ...(image.featureData || {}),
              smartCulling: {
                taskId: activeTaskId || undefined,
                status: item.status,
                rating: item.rating,
                colorLabel: item.colorLabel,
                reasonCodes: item.reasonCodes,
                reasonText: item.reasonText,
                degraded: item.degraded,
                groupId: item.groupId,
                groupRank: item.groupRank,
                groupSize: item.groupSize,
              },
            },
          };
        }),
      };
    });
  };

  const handleApply = async (itemsToApply = items) => {
    if (!activeTaskId) return;
    if (result?.previewOnly) {
      setSmartCulling({ error: '当前任务为仅分析不写入模式，不能应用结果。' });
      return;
    }
    setIsApplying(true);
    try {
      const applyResult = await invoke<SmartCullingApplyResult>(Invokes.SmartCullingApplyTaskResult, {
        taskId: activeTaskId,
        items: itemsToApply,
      });
      setItems(itemsToApply);
      updateImageListAfterItems(itemsToApply, applyResult.appliedPaths);
      setSmartCulling((state) => ({
        result: state.result
          ? { ...state.result, status: 'applied', items: itemsToApply, reportPath: applyResult.reportPath }
          : state.result,
      }));
      window.dispatchEvent(new CustomEvent('smart-culling-applied', { detail: applyResult }));
      onBackToLibrary();
    } catch (error) {
      setSmartCulling({ error: String(error) });
    } finally {
      setIsApplying(false);
    }
  };

  const handleApplyOnly = async (status: SmartCullingStatus) => {
    await handleApply(buildApplyOnlyItems(status));
  };

  const handleDiscard = async () => {
    if (activeTaskId) {
      await invoke(Invokes.SmartCullingDiscardTaskResult, { taskId: activeTaskId }).catch(() => undefined);
    }
    setSmartCulling({ result: null, activeTaskId: null, progress: null, error: null });
    onBackToLibrary();
  };

  const handleExportReport = async () => {
    if (!activeTaskId) return;
    setIsExportingReport(true);
    try {
      const report = await invoke<SmartCullingReportResult>(Invokes.SmartCullingExportReportPdf, {
        params: { taskId: activeTaskId, items },
      });
      setSmartCulling((state) => ({
        result: state.result ? { ...state.result, reportPath: report.reportPath, items } : state.result,
      }));
      await open(report.reportPath);
    } catch (error) {
      setSmartCulling({ error: String(error) });
    } finally {
      setIsExportingReport(false);
    }
  };

  const handleUndo = async () => {
    if (!activeTaskId) return;
    setIsUndoing(true);
    try {
      const undoResult = await invoke<SmartCullingUndoResult>(Invokes.SmartCullingUndoTask, { taskId: activeTaskId });
      setSmartCulling((state) => ({
        result: state.result ? { ...state.result, status: 'revoked' } : state.result,
        error:
          undoResult.skipped > 0
            ? `已撤销 ${undoResult.restored} 张，跳过 ${undoResult.skipped} 张用户已修改照片。`
            : `已撤销 ${undoResult.restored} 张智能选图写入。`,
      }));
      await onLibraryRefresh?.();
      onBackToLibrary();
    } catch (error) {
      setSmartCulling({ error: String(error) });
    } finally {
      setIsUndoing(false);
    }
  };

  if (isLoading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="animate-spin text-accent" size={36} />
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col h-full min-w-0 bg-bg-secondary rounded-lg overflow-hidden">
      <header className="p-4 border-b border-border-color flex items-center justify-between gap-4">
        <div className="flex items-center gap-3 min-w-0">
          <Button className="bg-surface text-text-primary h-10 w-10 p-0" onClick={onBackToLibrary}>
            <ArrowLeft size={18} />
          </Button>
          <div className="min-w-0">
            <Text variant={TextVariants.headline}>智能选图复核</Text>
            <Text variant={TextVariants.small} color={TextColors.secondary}>
              分析 {summary.analyzed} 张，跳过 {summary.skipped} 张，精选 {summary.selected} 张，待确认 {summary.review}{' '}
              张，淘汰建议 {summary.rejectSuggestion} 张
            </Text>
          </div>
        </div>
        <div className="flex items-center gap-2 flex-wrap justify-end">
          <Button
            className="bg-surface text-text-primary"
            onClick={handleExportReport}
            disabled={isExportingReport || items.length === 0}
          >
            {isExportingReport ? <Loader2 size={16} className="animate-spin" /> : <FileText size={16} />}
            导出报告
          </Button>
          <Button
            className="bg-surface text-text-primary"
            onClick={handleUndo}
            disabled={isUndoing || result?.status !== 'applied'}
          >
            {isUndoing ? <Loader2 size={16} className="animate-spin" /> : <RotateCcw size={16} />}
            撤销
          </Button>
          <Button
            className="bg-surface text-text-primary"
            onClick={() => handleApplyOnly('selected')}
            disabled={readOnly || isApplying || items.length === 0 || Boolean(result?.previewOnly)}
          >
            <Star size={16} />
            只应用精选
          </Button>
          <Button
            className="bg-surface text-text-primary"
            onClick={() => handleApplyOnly('reject_suggestion')}
            disabled={readOnly || isApplying || items.length === 0 || Boolean(result?.previewOnly)}
          >
            <X size={16} />
            只应用淘汰建议
          </Button>
          <Button className="bg-surface text-text-primary" onClick={handleDiscard} disabled={isApplying || readOnly}>
            <X size={16} />
            拒绝全部
          </Button>
          <Button
            onClick={() => handleApply()}
            disabled={readOnly || isApplying || items.length === 0 || Boolean(result?.previewOnly)}
          >
            {isApplying ? <Loader2 size={16} className="animate-spin" /> : <Check size={16} />}
            {result?.previewOnly ? '预览模式不可写入' : '接受全部'}
          </Button>
        </div>
      </header>

      {error && (
        <div className="mx-4 mt-4 rounded-md border border-red-500/40 bg-red-500/10 p-3">
          <Text color={TextColors.error}>{error}</Text>
        </div>
      )}

      <div className="p-4 border-b border-border-color flex items-center gap-3 flex-wrap">
        <div className="w-44">
          <Dropdown
            value={statusFilter}
            onChange={setStatusFilter}
            options={[
              { value: 'all', label: '全部状态' },
              { value: 'selected', label: '精选' },
              { value: 'review', label: '待确认' },
              { value: 'reject_suggestion', label: '淘汰建议' },
              { value: 'skipped', label: '跳过' },
              { value: 'failed', label: '失败' },
            ]}
          />
        </div>
        <div className="w-44">
          <Dropdown value={reasonFilter} onChange={setReasonFilter} options={reasonOptions} />
        </div>
        <div className="w-36">
          <Dropdown
            value={ratingFilter}
            onChange={setRatingFilter}
            options={[
              { value: 'all', label: '全部星级' },
              { value: '5', label: '5 星' },
              { value: '4', label: '4 星' },
              { value: '3', label: '3 星' },
              { value: '2', label: '2 星' },
              { value: '1', label: '1 星' },
            ]}
          />
        </div>
        <div className="w-36">
          <Dropdown
            value={colorFilter}
            onChange={setColorFilter}
            options={[{ value: 'all', label: '全部颜色' }, ...COLOR_OPTIONS]}
          />
        </div>
        <Text variant={TextVariants.small} color={TextColors.secondary}>
          淘汰建议不会删除文件，只会写入智能选图建议。
        </Text>
      </div>

      <div className="flex-1 overflow-y-auto custom-scrollbar p-4 space-y-5">
        {groupedItems.groups.map(([groupId, group]) => {
          const expanded = expandedGroups[groupId] || false;
          const visibleGroupItems = expanded ? group : group.slice(0, 1);
          return (
            <section key={groupId} className="space-y-3">
              <div className="flex items-center justify-between">
                <button
                  type="button"
                  className="flex items-center gap-2 text-text-primary"
                  onClick={() => setExpandedGroups((state) => ({ ...state, [groupId]: !expanded }))}
                >
                  {expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
                  <Text variant={TextVariants.label} weight={TextWeights.semibold}>
                    相似组 · 显示最优 {group.length > 1 ? `，折叠 ${group.length - 1} 张` : ''}
                  </Text>
                </button>
                <div className="flex items-center gap-2">
                  <Text variant={TextVariants.small} color={TextColors.secondary}>
                    可展开对比，并手动设为最优
                  </Text>
                  {!readOnly && (
                    <button
                      type="button"
                      className="text-xs rounded-md bg-bg-primary px-2 py-1 text-text-secondary hover:text-text-primary"
                      onClick={() => acceptGroupBestOnly(groupId)}
                    >
                      接受本组
                    </button>
                  )}
                </div>
              </div>
              <div className="grid grid-cols-1 xl:grid-cols-2 gap-3">
                {visibleGroupItems.map((item) => (
                  <ReviewCard
                    key={item.path}
                    item={item}
                    readOnly={readOnly}
                    onChange={updateItem}
                    onPromoteBest={promoteGroupBest}
                  />
                ))}
              </div>
            </section>
          );
        })}

        {groupedItems.singles.length > 0 && (
          <section className="space-y-3">
            <Text variant={TextVariants.label} weight={TextWeights.semibold}>
              单张结果
            </Text>
            <div className="grid grid-cols-1 xl:grid-cols-2 gap-3">
              {groupedItems.singles.map((item) => (
                <ReviewCard key={item.path} item={item} readOnly={readOnly} onChange={updateItem} />
              ))}
            </div>
          </section>
        )}
      </div>
    </div>
  );
}
