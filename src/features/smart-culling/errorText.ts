import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { FailureItem, SmartCullingCommandError } from './types';

const zhFailures: Record<string, string> = {
  manual_protected: '已有人工处理结果，已保护并跳过',
  excluded_format: '不支持 GIF、TIFF/TIF，已跳过',
  ambiguous_pair: '同名 RAW/JPEG 组合不明确，未自动配对',
  scan_failed: '读取图片或 .rrdata 失败',
  render_failed: '生成当前编辑状态预览失败',
  analysis_failed: '本地模型分析失败，已跳过',
  asset_changed: '原始照片在分析后发生变化，需要重新开始筛选',
  baseline_conflict: '.rrdata 在任务期间发生变化，请确认后重试',
  invalid_result: '筛选结果校验失败，未写入',
  io_error: '读取或写入 .rrdata 失败',
  nothing_to_write: '没有可写入的已采用结果',
};

const enFailures: Record<string, string> = {
  manual_protected: 'Protected manual result; skipped',
  excluded_format: 'GIF and TIFF/TIF are unsupported; skipped',
  ambiguous_pair: 'Matching RAW/JPEG members are ambiguous; pairing was skipped',
  scan_failed: 'Could not read the photo or its .rrdata',
  render_failed: 'Could not render the current edited state',
  analysis_failed: 'Local model analysis failed; skipped',
  asset_changed: 'The original photo changed after analysis; start a new cull',
  baseline_conflict: 'The .rrdata changed during this task; review it before retrying',
  invalid_result: 'The culling result failed validation and was not written',
  io_error: 'Could not read or write the .rrdata',
  nothing_to_write: 'There are no adopted results to write',
};

const zhCommands: Record<string, string> = {
  gateway_failed: '智能选图服务暂时不可用',
  status_failed: '读取智能选图状态失败',
  inspect_failed: '检查文件夹失败',
  detect_people_failed: '识别照片人物失败',
  start_failed: '启动智能选图失败',
  cancel_failed: '取消任务失败',
  update_review_failed: '保存复核修改失败',
  confirm_failed: '确认写入失败',
  retry_failures_failed: '重试失败项失败',
  reconcile_manual_failed: '记录人工处理来源失败',
  abandon_failed: '清除本次任务失败',
  unexpected_error: '发生未预期错误',
};

const enCommands: Record<string, string> = {
  gateway_failed: 'Smart Culling is temporarily unavailable',
  status_failed: 'Could not read Smart Culling status',
  inspect_failed: 'Could not inspect this folder',
  detect_people_failed: 'Could not detect people in this photo',
  start_failed: 'Could not start Smart Culling',
  cancel_failed: 'Could not cancel the task',
  update_review_failed: 'Could not save the review change',
  confirm_failed: 'Could not confirm and write results',
  retry_failures_failed: 'Could not retry failed items',
  reconcile_manual_failed: 'Could not record the manual source',
  abandon_failed: 'Could not clear this task',
  unexpected_error: 'An unexpected error occurred',
};

function useChineseLanguage() {
  const { i18n } = useTranslation();
  return i18n.resolvedLanguage?.toLowerCase().startsWith('zh') ?? false;
}

export function useSmartCullingFailureText() {
  const isChinese = useChineseLanguage();
  return useCallback(
    (failure: Pick<FailureItem, 'code' | 'detail'>) =>
      (isChinese ? zhFailures : enFailures)[failure.code] ?? failure.detail,
    [isChinese],
  );
}

export function useSmartCullingCommandErrorText() {
  const isChinese = useChineseLanguage();
  return useCallback(
    (error: SmartCullingCommandError) => (isChinese ? zhCommands : enCommands)[error.code] ?? error.detail,
    [isChinese],
  );
}
