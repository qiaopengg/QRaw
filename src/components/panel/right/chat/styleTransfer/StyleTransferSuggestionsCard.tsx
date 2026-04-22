import React from 'react';
import { Save } from 'lucide-react';
import Slider from '../../../../ui/Slider';
import { Adjustments } from '../../../../../utils/adjustments';
import { ChatMessage, StyleConstraintAction } from '../types';

const HSL_COLOR_LABELS: Record<string, string> = {
  reds: '红色',
  oranges: '橙色',
  yellows: '黄色',
  greens: '绿色',
  aquas: '青色',
  blues: '蓝色',
  purples: '紫色',
  magentas: '洋红',
};

type SliderChangeEvent = { target: { value: number | string } } | React.ChangeEvent<HTMLInputElement>;

interface StyleTransferSuggestionsCardProps {
  addPreset(
    name: string,
    folder?: string | null,
    includeMasks?: boolean,
    includeCropTransform?: boolean,
    updateExisting?: boolean,
  ): void;
  adjustments: Adjustments;
  applyAllSuggestions(message: ChatMessage): void;
  applyConstraintActions(messageId: string, actions: StyleConstraintAction[]): void;
  handleHslSliderChange(
    messageId: string,
    color: string,
    channel: 'hue' | 'saturation' | 'luminance',
    event: SliderChangeEvent,
  ): void;
  handleSliderChange(messageId: string, key: string, event: SliderChangeEvent): void;
  message: ChatMessage;
  t(key: string): string;
}

export function StyleTransferSuggestionsCard({
  addPreset,
  adjustments,
  applyAllSuggestions,
  applyConstraintActions,
  handleHslSliderChange,
  handleSliderChange,
  message,
  t,
}: StyleTransferSuggestionsCardProps) {
  if (!message.adjustments || message.adjustments.length === 0) {
    return null;
  }

  return (
    <div className="w-full bg-surface/50 rounded-lg p-2 space-y-2 border border-surface">
      {message.styleDebug && (
        <div className="rounded border border-surface bg-bg-primary/60 px-2 py-1.5 space-y-1">
          <div className="flex items-center justify-between text-[9px] text-text-secondary">
            <span>误差分解</span>
            <span>
              收敛 {(message.styleDebug.improvement_ratio * 100).toFixed(1)}% · 当前短板{' '}
              {message.styleDebug.dominant_error}
            </span>
          </div>
          <div className="text-[9px] text-text-secondary/70">
            自动二次微调 {message.styleDebug.auto_refine_rounds} 轮
          </div>
          {message.styleDebug.scene_profile && (
            <div className="rounded border border-blue-400/20 px-1.5 py-1 space-y-0.5">
              <div className="text-[9px] text-blue-300/90">
                场景判定 {message.styleDebug.scene_profile.reference_tonal_style} →{' '}
                {message.styleDebug.scene_profile.current_tonal_style}
              </div>
              <div className="grid grid-cols-3 gap-2 text-[9px] text-text-secondary/80">
                <span>明暗 {message.styleDebug.scene_profile.tonal_gain.toFixed(2)}</span>
                <span>高光 {message.styleDebug.scene_profile.highlight_gain.toFixed(2)}</span>
                <span>阴影 {message.styleDebug.scene_profile.shadow_gain.toFixed(2)}</span>
                <span>色彩上限 {message.styleDebug.scene_profile.chroma_limit.toFixed(2)}</span>
                <span>色彩护栏 {message.styleDebug.scene_profile.chroma_guard_floor.toFixed(2)}</span>
                <span>色准回正 {message.styleDebug.scene_profile.color_residual_gain.toFixed(2)}</span>
              </div>
            </div>
          )}
          <div className="grid grid-cols-2 gap-x-3 gap-y-0.5 text-[9px] text-text-secondary/80">
            <span>
              接近度总分 {message.styleDebug.proximity_before.overall.toFixed(1)} →{' '}
              {message.styleDebug.proximity_after.overall.toFixed(1)}
            </span>
            <span>
              影调 {message.styleDebug.proximity_before.tonal.toFixed(1)} →{' '}
              {message.styleDebug.proximity_after.tonal.toFixed(1)}
            </span>
            <span>
              色彩 {message.styleDebug.proximity_before.color.toFixed(1)} →{' '}
              {message.styleDebug.proximity_after.color.toFixed(1)}
            </span>
            <span>
              肤色 {message.styleDebug.proximity_before.skin.toFixed(1)} →{' '}
              {message.styleDebug.proximity_after.skin.toFixed(1)}
            </span>
            <span>
              高光安全 {message.styleDebug.proximity_before.highlight.toFixed(1)} →{' '}
              {message.styleDebug.proximity_after.highlight.toFixed(1)}
            </span>
          </div>
          <div className="grid grid-cols-2 gap-x-3 gap-y-0.5 text-[9px] text-text-secondary/80">
            <span>
              影调 {message.styleDebug.before.tonal.toFixed(2)} → {message.styleDebug.after.tonal.toFixed(2)}
            </span>
            <span>
              色彩 {message.styleDebug.before.color.toFixed(2)} → {message.styleDebug.after.color.toFixed(2)}
            </span>
            <span>
              肤色 {message.styleDebug.before.skin.toFixed(2)} → {message.styleDebug.after.skin.toFixed(2)}
            </span>
            <span>
              过曝 {message.styleDebug.before.highlight_penalty.toFixed(2)} →{' '}
              {message.styleDebug.after.highlight_penalty.toFixed(2)}
            </span>
          </div>
          {message.styleDebug.suggested_actions.length > 0 && (
            <div className="space-y-0.5">
              {message.styleDebug.suggested_actions.map((action) => (
                <div key={action.key} className="text-[9px] text-text-secondary/75">
                  {action.label} {action.recommended_delta >= 0 ? '+' : ''}
                  {action.recommended_delta.toFixed(action.key === 'exposure' ? 2 : 1)} · {action.reason}
                </div>
              ))}
            </div>
          )}
          {(message.styleDebug.blocked_items ?? []).length > 0 && (
            <div className="space-y-1">
              {(message.styleDebug.blocked_items ?? []).map((item) => (
                <div key={`${item.category}-${item.reason}`} className="rounded border border-amber-400/20 px-1.5 py-1">
                  <div className="flex items-center justify-between text-[9px]">
                    <span className="text-amber-300/90">
                      {item.label} · 命中 {item.hit_count} 次 · 强度 {item.severity.toFixed(2)}
                    </span>
                    <button
                      onClick={() => applyConstraintActions(message.id, item.actions)}
                      className="text-blue-300 hover:text-blue-200 transition-colors"
                    >
                      一键微调
                    </button>
                  </div>
                  <div className="text-[9px] text-amber-300/80">受限原因：{item.reason}</div>
                </div>
              ))}
            </div>
          )}
          {(message.styleDebug.blocked_items ?? []).length === 0 && message.styleDebug.blocked_reasons.length > 0 && (
            <div className="space-y-0.5">
              {message.styleDebug.blocked_reasons.map((reason, idx) => (
                <div key={`${reason}-${idx}`} className="text-[9px] text-amber-300/80">
                  受限原因：{reason}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {message.constraintDebug && (
        <div className="rounded border border-surface bg-bg-primary/60 px-2 py-1.5 space-y-1">
          <div className="flex items-center justify-between text-[9px] text-text-secondary">
            <span>动态约束</span>
            <span>
              来源 {message.constraintDebug.window.source} · 命中 {message.constraintDebug.clamp_count} 项
            </span>
          </div>
          <div className="grid grid-cols-3 gap-2 text-[9px] text-text-secondary/80">
            <span>高光风险 {message.constraintDebug.window.highlight_risk.toFixed(2)}</span>
            <span>阴影风险 {message.constraintDebug.window.shadow_risk.toFixed(2)}</span>
            <span>饱和风险 {message.constraintDebug.window.saturation_risk.toFixed(2)}</span>
          </div>
          {message.constraintDebug.clamps.slice(0, 3).map((clamp) => (
            <div key={`${clamp.key}-${clamp.original}`} className="text-[9px] text-text-secondary/75">
              {clamp.label} {clamp.original.toFixed(clamp.key === 'exposure' ? 2 : 1)} →{' '}
              {clamp.clamped.toFixed(clamp.key === 'exposure' ? 2 : 1)} · {clamp.reason}
            </div>
          ))}
        </div>
      )}

      <div className="flex items-center justify-between">
        <span className="text-[10px] text-text-secondary">{t('chat.suggestions')}</span>
        <div className="flex items-center gap-3">
          <button
            onClick={() => {
              applyAllSuggestions(message);
              const name = prompt('请输入预设名称', 'AI 预设');
              if (name) addPreset(name, null, false, false, false);
            }}
            className="flex items-center gap-1 text-[10px] text-purple-400 hover:text-purple-300 transition-colors"
            title="将当前调整保存为预设"
          >
            <Save size={10} />
            保存为预设
          </button>
          <button
            onClick={() => applyAllSuggestions(message)}
            className="text-[10px] text-blue-400 hover:text-blue-300 transition-colors"
          >
            {t('chat.applyAll')}
          </button>
        </div>
      </div>

      {message.adjustments.map((suggestion) => (
        <div key={suggestion.key} className="space-y-0.5">
          {suggestion.reason && <p className="text-[9px] text-text-secondary opacity-60">{suggestion.reason}</p>}
          {suggestion.key === 'hsl' &&
          suggestion.complex_value !== undefined &&
          typeof suggestion.complex_value === 'object' ? (
            <div className="rounded border border-surface bg-surface/30 px-2 py-1.5 space-y-1">
              <div className="flex items-center justify-between">
                <span className="text-[10px] text-text-primary">{suggestion.label || '颜色混合器'}</span>
                <span className="text-[9px] text-purple-400 bg-purple-500/10 px-1.5 py-0.5 rounded">HSL</span>
              </div>
              {Object.entries(
                suggestion.complex_value as Record<string, Partial<Adjustments['hsl'][keyof Adjustments['hsl']]>>,
              ).map(([color]) => (
                <div key={color} className="space-y-0.5">
                  <div className="text-[9px] text-text-secondary/80">{HSL_COLOR_LABELS[color] || color}</div>
                  <Slider
                    label="色相"
                    min={-100}
                    max={100}
                    step={1}
                    value={adjustments.hsl[color]?.hue ?? 0}
                    onChange={(event) => handleHslSliderChange(message.id, color, 'hue', event)}
                  />
                  <Slider
                    label="饱和度"
                    min={-100}
                    max={100}
                    step={1}
                    value={adjustments.hsl[color]?.saturation ?? 0}
                    onChange={(event) => handleHslSliderChange(message.id, color, 'saturation', event)}
                  />
                  <Slider
                    label="明度"
                    min={-100}
                    max={100}
                    step={1}
                    value={adjustments.hsl[color]?.luminance ?? 0}
                    onChange={(event) => handleHslSliderChange(message.id, color, 'luminance', event)}
                  />
                </div>
              ))}
            </div>
          ) : suggestion.complex_value !== undefined ? (
            <div className="flex items-center justify-between bg-surface/50 rounded px-2 py-1.5 border border-surface">
              <span className="text-[10px] text-text-primary">{suggestion.label}</span>
              <span className="text-[9px] text-purple-400 bg-purple-500/10 px-1.5 py-0.5 rounded">已应用高级映射</span>
            </div>
          ) : (
            <Slider
              label={suggestion.label}
              min={suggestion.min}
              max={suggestion.max}
              step={suggestion.key === 'exposure' ? 0.01 : 1}
              value={
                (adjustments[suggestion.key as keyof Adjustments] as number) ??
                message.appliedValues?.[suggestion.key] ??
                suggestion.value
              }
              onChange={(event) => handleSliderChange(message.id, suggestion.key, event)}
            />
          )}
        </div>
      ))}
    </div>
  );
}
