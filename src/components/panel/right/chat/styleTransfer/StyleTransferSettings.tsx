import React from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, SlidersHorizontal } from 'lucide-react';
import {
  STYLE_TRANSFER_PRESET_OPTIONS,
  StyleTransferModelStatusResponse,
  StyleTransferPreset,
  StyleTransferStrategyMode,
} from '../types';

interface StyleTransferSettingsProps {
  enableStyleTransferAutoRefine: boolean;
  enableStyleTransferExpertPreset: boolean;
  enableStyleTransferFeatureMapping: boolean;
  enableStyleTransferLut: boolean;
  enableStyleTransferVlm: boolean;
  highlightGuardInput: string;
  menuOpen: boolean;
  menuRef: React.RefObject<HTMLDivElement | null>;
  pureStyleTransfer: boolean;
  saveStyleTransferConfig(
    key: 'styleTransferStrength' | 'styleTransferHighlightGuard' | 'styleTransferSkinProtect',
    raw: string,
  ): string;
  setEnableStyleTransferAutoRefine(value: React.SetStateAction<boolean>): void;
  setEnableStyleTransferExpertPreset(value: React.SetStateAction<boolean>): void;
  setEnableStyleTransferFeatureMapping(value: React.SetStateAction<boolean>): void;
  setEnableStyleTransferLut(value: React.SetStateAction<boolean>): void;
  setEnableStyleTransferVlm(value: React.SetStateAction<boolean>): void;
  setHighlightGuardInput(value: React.SetStateAction<string>): void;
  setMenuOpen(value: React.SetStateAction<boolean>): void;
  setPureStyleTransfer(value: React.SetStateAction<boolean>): void;
  setSkinProtectInput(value: React.SetStateAction<string>): void;
  setStyleStrengthInput(value: React.SetStateAction<string>): void;
  skinProtectInput: string;
  isPreparingStyleTransferModels: boolean;
  prepareStyleTransferModels(): void | Promise<unknown>;
  styleStrengthInput: string;
  styleTransferModelStatus: StyleTransferModelStatusResponse | null;
  styleTransferPreset: StyleTransferPreset;
  styleTransferStrategyMode: StyleTransferStrategyMode;
  updateStyleTransferStrategyMode(mode: StyleTransferStrategyMode): void;
  updateStyleTransferPreset(preset: StyleTransferPreset): void;
}

export function StyleTransferSettings({
  enableStyleTransferAutoRefine,
  enableStyleTransferExpertPreset,
  enableStyleTransferFeatureMapping,
  enableStyleTransferLut,
  enableStyleTransferVlm,
  highlightGuardInput,
  menuOpen,
  menuRef,
  pureStyleTransfer,
  saveStyleTransferConfig,
  setEnableStyleTransferAutoRefine,
  setEnableStyleTransferExpertPreset,
  setEnableStyleTransferFeatureMapping,
  setEnableStyleTransferLut,
  setEnableStyleTransferVlm,
  setHighlightGuardInput,
  setMenuOpen,
  setPureStyleTransfer,
  setSkinProtectInput,
  setStyleStrengthInput,
  skinProtectInput,
  isPreparingStyleTransferModels,
  prepareStyleTransferModels,
  styleStrengthInput,
  styleTransferModelStatus,
  styleTransferPreset,
  styleTransferStrategyMode,
  updateStyleTransferStrategyMode,
  updateStyleTransferPreset,
}: StyleTransferSettingsProps) {
  const { t } = useTranslation();

  return (
    <div className="relative" ref={menuRef}>
      <button
        onClick={() => setMenuOpen((value) => !value)}
        className="flex items-center gap-1 px-2 py-1 rounded text-[10px] text-text-secondary hover:text-text-primary hover:bg-surface transition-colors"
        title="风格迁移参数"
      >
        <SlidersHorizontal size={10} />
        <span>{t('chat.transferSettings')}</span>
        <ChevronDown size={10} className={`transition-transform ${menuOpen ? 'rotate-180' : ''}`} />
      </button>
      {menuOpen && (
        <div className="absolute right-0 top-full mt-1 w-72 bg-surface/95 backdrop-blur-md rounded-lg shadow-xl p-2 z-50 border border-surface space-y-2">
          <div className="space-y-2">
            <div className="space-y-1">
              <div className="text-[9px] text-text-secondary/85">{t('chat.styleTransferStrategyMode')}</div>
              <div className="grid grid-cols-2 gap-1">
                {(['safe', 'strong'] as const).map((mode) => (
                  <button
                    key={mode}
                    onClick={() => updateStyleTransferStrategyMode(mode)}
                    className={`rounded px-1.5 py-1.5 text-[10px] transition-colors ${
                      styleTransferStrategyMode === mode
                        ? 'bg-blue-500/20 text-blue-300'
                        : 'bg-bg-primary text-text-secondary hover:text-text-primary'
                    }`}
                  >
                    {mode === 'safe' ? t('chat.styleTransferStrategySafe') : t('chat.styleTransferStrategyStrong')}
                  </button>
                ))}
              </div>
            </div>

            <div className="space-y-1 rounded border border-surface bg-bg-primary/70 p-2">
              <div className="flex items-center justify-between gap-2">
                <div className="min-w-0">
                  <div className="text-[9px] text-text-secondary/85">{t('chat.styleTransferModelStatusTitle')}</div>
                  <div className="text-[10px] text-text-primary">
                    {styleTransferModelStatus
                      ? `${styleTransferModelStatus.readyCount}/${styleTransferModelStatus.requiredCount} ${t('chat.styleTransferModelReadySuffix')}`
                      : t('chat.styleTransferModelStatusUnknown')}
                  </div>
                </div>
                <button
                  onClick={() => void prepareStyleTransferModels()}
                  disabled={isPreparingStyleTransferModels}
                  className="rounded px-2 py-1 text-[10px] bg-blue-500/20 text-blue-300 hover:bg-blue-500/30 transition-colors disabled:opacity-50"
                >
                  {isPreparingStyleTransferModels
                    ? t('chat.styleTransferModelPreparing')
                    : t('chat.styleTransferPrepareModels')}
                </button>
              </div>
              <div className="text-[9px] text-text-secondary/70">
                {styleTransferModelStatus?.fullReady
                  ? t('chat.styleTransferModelStatusReady')
                  : styleTransferModelStatus?.requiredReady
                    ? t('chat.styleTransferModelStatusDegraded')
                    : t('chat.styleTransferModelStatusPending')}
              </div>
              {styleTransferModelStatus?.models?.length ? (
                <div className="max-h-24 overflow-y-auto space-y-1 pt-1">
                  {styleTransferModelStatus.models.map((model) => (
                    <div
                      key={model.id}
                      className="flex items-center justify-between gap-2 rounded bg-surface/40 px-1.5 py-1 text-[9px]"
                    >
                      <div className="min-w-0 flex items-center gap-1.5">
                        <span className="truncate text-text-secondary">{model.name}</span>
                        <span
                          className={`shrink-0 rounded px-1 py-0.5 ${
                            model.required
                              ? 'bg-blue-500/10 text-blue-300'
                              : 'bg-text-secondary/10 text-text-secondary/80'
                          }`}
                        >
                          {model.required ? '必需' : '可选'}
                        </span>
                      </div>
                      <span className={model.ready ? 'text-emerald-300' : model.required ? 'text-amber-300' : 'text-text-secondary/70'}>
                        {model.ready ? t('chat.styleTransferArtifactReady') : model.required ? t('chat.styleTransferArtifactPending') : '不影响主流程'}
                      </span>
                    </div>
                  ))}
                </div>
              ) : null}
            </div>

            <div className="space-y-1">
              <div className="text-[9px] text-text-secondary/85">{t('chat.styleTransferPreset')}</div>
              <div className="grid grid-cols-3 gap-1">
                {STYLE_TRANSFER_PRESET_OPTIONS.map((option) => (
                  <button
                    key={option.value}
                    onClick={() => updateStyleTransferPreset(option.value)}
                    className={`rounded px-1.5 py-1.5 text-[10px] transition-colors ${
                      styleTransferPreset === option.value
                        ? 'bg-blue-500/20 text-blue-300'
                        : 'bg-bg-primary text-text-secondary hover:text-text-primary'
                    }`}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            </div>

            <label className="space-y-1 block">
              <span className="text-[9px] text-text-secondary/85">{t('chat.styleTransferStrengthLabel')}</span>
              <input
                type="number"
                min={0.5}
                max={2.0}
                step={0.05}
                value={styleStrengthInput}
                onChange={(event) => setStyleStrengthInput(event.target.value)}
                onBlur={() =>
                  setStyleStrengthInput(saveStyleTransferConfig('styleTransferStrength', styleStrengthInput))
                }
                className="w-full bg-bg-primary rounded px-1.5 py-1 text-[10px] text-text-primary outline-none border border-surface focus:border-blue-500/50"
              />
            </label>

            <label className="flex items-center justify-between gap-2 text-[10px] text-text-secondary/85">
              <span>{t('chat.styleTransferPureAlgorithm')}</span>
              <input
                type="checkbox"
                checked={pureStyleTransfer}
                onChange={(event) => setPureStyleTransfer(event.target.checked)}
                className="accent-blue-500"
              />
            </label>

            {!pureStyleTransfer && (
              <div className="space-y-1">
                <label className="flex items-center justify-between gap-2 text-[10px] text-text-secondary/85">
                  <span>{t('chat.styleTransferEnableLut')}</span>
                  <input
                    type="checkbox"
                    checked={enableStyleTransferLut}
                    onChange={(event) => setEnableStyleTransferLut(event.target.checked)}
                    className="accent-blue-500"
                  />
                </label>
                <label className="flex items-center justify-between gap-2 text-[10px] text-text-secondary/85">
                  <span>{t('chat.styleTransferEnableExpertPreset')}</span>
                  <input
                    type="checkbox"
                    checked={enableStyleTransferExpertPreset}
                    onChange={(event) => setEnableStyleTransferExpertPreset(event.target.checked)}
                    className="accent-blue-500"
                  />
                </label>
                <label className="flex items-center justify-between gap-2 text-[10px] text-text-secondary/85">
                  <span>{t('chat.styleTransferEnableFeatureMapping')}</span>
                  <input
                    type="checkbox"
                    checked={enableStyleTransferFeatureMapping}
                    onChange={(event) => setEnableStyleTransferFeatureMapping(event.target.checked)}
                    className="accent-blue-500"
                  />
                </label>
                <label className="flex items-center justify-between gap-2 text-[10px] text-text-secondary/85">
                  <span>{t('chat.styleTransferEnableAutoRefine')}</span>
                  <input
                    type="checkbox"
                    checked={enableStyleTransferAutoRefine}
                    onChange={(event) => setEnableStyleTransferAutoRefine(event.target.checked)}
                    className="accent-blue-500"
                  />
                </label>
                <label className="flex items-center justify-between gap-2 text-[10px] text-text-secondary/85">
                  <span>{t('chat.styleTransferEnableVlm')}</span>
                  <input
                    type="checkbox"
                    checked={enableStyleTransferVlm}
                    onChange={(event) => setEnableStyleTransferVlm(event.target.checked)}
                    className="accent-blue-500"
                  />
                </label>
              </div>
            )}

            <label className="space-y-1 block">
              <span className="text-[9px] text-text-secondary/85">{t('chat.styleTransferHighlightGuardLabel')}</span>
              <input
                type="number"
                min={0.5}
                max={2.0}
                step={0.05}
                value={highlightGuardInput}
                onChange={(event) => setHighlightGuardInput(event.target.value)}
                onBlur={() =>
                  setHighlightGuardInput(
                    saveStyleTransferConfig('styleTransferHighlightGuard', highlightGuardInput),
                  )
                }
                className="w-full bg-bg-primary rounded px-1.5 py-1 text-[10px] text-text-primary outline-none border border-surface focus:border-blue-500/50"
              />
            </label>

            <label className="space-y-1 block">
              <span className="text-[9px] text-text-secondary/85">{t('chat.styleTransferSkinProtectLabel')}</span>
              <input
                type="number"
                min={0.5}
                max={2.0}
                step={0.05}
                value={skinProtectInput}
                onChange={(event) => setSkinProtectInput(event.target.value)}
                onBlur={() =>
                  setSkinProtectInput(saveStyleTransferConfig('styleTransferSkinProtect', skinProtectInput))
                }
                className="w-full bg-bg-primary rounded px-1.5 py-1 text-[10px] text-text-primary outline-none border border-surface focus:border-blue-500/50"
              />
            </label>
          </div>
        </div>
      )}
    </div>
  );
}
