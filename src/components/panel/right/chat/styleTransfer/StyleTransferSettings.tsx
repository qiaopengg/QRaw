import React from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, SlidersHorizontal } from 'lucide-react';
import {
  STYLE_TRANSFER_EXPORT_FORMAT_OPTIONS,
  STYLE_TRANSFER_PRESET_OPTIONS,
  StyleTransferExportFormat,
  StyleTransferModeSetting,
  StyleTransferPreset,
  StyleTransferServiceStatus,
} from '../types';
import { normalizeServiceUrl } from './utils';

interface StyleTransferSettingsProps {
  checkingStyleTransferService: boolean;
  checkStyleTransferService(): void;
  commitStyleTransferServiceUrl(): void;
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
  setStyleTransferServiceUrl(value: React.SetStateAction<string>): void;
  skinProtectInput: string;
  styleStrengthInput: string;
  styleTransferAllowFallback: boolean;
  styleTransferEnableRefiner: boolean;
  styleTransferExportFormat: StyleTransferExportFormat;
  styleTransferMode: StyleTransferModeSetting;
  styleTransferPreset: StyleTransferPreset;
  styleTransferServiceStatus: StyleTransferServiceStatus | null;
  styleTransferServiceUrl: string;
  updateStyleTransferAllowFallback(enabled: boolean): void;
  updateStyleTransferEnableRefiner(enabled: boolean): void;
  updateStyleTransferExportFormat(format: StyleTransferExportFormat): void;
  updateStyleTransferMode(mode: StyleTransferModeSetting): void;
  updateStyleTransferPreset(preset: StyleTransferPreset): void;
}

export function StyleTransferSettings({
  checkingStyleTransferService,
  checkStyleTransferService,
  commitStyleTransferServiceUrl,
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
  setStyleTransferServiceUrl,
  skinProtectInput,
  styleStrengthInput,
  styleTransferAllowFallback,
  styleTransferEnableRefiner,
  styleTransferExportFormat,
  styleTransferMode,
  styleTransferPreset,
  styleTransferServiceStatus,
  styleTransferServiceUrl,
  updateStyleTransferAllowFallback,
  updateStyleTransferEnableRefiner,
  updateStyleTransferExportFormat,
  updateStyleTransferMode,
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
          <div className="space-y-1">
            <div className="text-[9px] text-text-secondary/85">{t('chat.styleTransferModeLabel')}</div>
            <div className="grid grid-cols-2 gap-1">
              <button
                onClick={() => updateStyleTransferMode('analysis')}
                className={`rounded px-2 py-1.5 text-[10px] transition-colors ${
                  styleTransferMode === 'analysis'
                    ? 'bg-blue-500/20 text-blue-300'
                    : 'bg-bg-primary text-text-secondary hover:text-text-primary'
                }`}
              >
                {t('chat.styleTransferModeAnalysis')}
              </button>
              <button
                onClick={() => updateStyleTransferMode('generativePreview')}
                className={`rounded px-2 py-1.5 text-[10px] transition-colors ${
                  styleTransferMode === 'generativePreview'
                    ? 'bg-purple-500/20 text-purple-300'
                    : 'bg-bg-primary text-text-secondary hover:text-text-primary'
                }`}
              >
                {t('chat.styleTransferModeGenerative')}
              </button>
            </div>
            <div className="grid grid-cols-2 gap-1 text-[9px] text-text-secondary/70">
              <span>{t('chat.styleTransferAnalysisCost')}</span>
              <span>{t('chat.styleTransferPreviewCost')}</span>
            </div>
          </div>

          {styleTransferMode === 'generativePreview' ? (
            <div className="space-y-2">
              <div className="rounded border border-surface bg-bg-primary/60 px-2 py-1.5">
                <div className="flex items-center justify-between text-[9px] text-text-secondary">
                  <span>{t('chat.styleTransferServiceStatus')}</span>
                  <span
                    className={
                      styleTransferServiceStatus?.reachable && styleTransferServiceStatus.ready
                        ? 'text-green-300'
                        : 'text-amber-300'
                    }
                  >
                    {checkingStyleTransferService
                      ? t('chat.checking')
                      : styleTransferServiceStatus?.reachable && styleTransferServiceStatus.ready
                        ? t('chat.serviceReady')
                        : t('chat.serviceUnavailable')}
                  </span>
                </div>
                <div className="mt-1 text-[9px] text-text-secondary/75 break-all">
                  {styleTransferServiceStatus?.serviceUrl || normalizeServiceUrl(styleTransferServiceUrl)}
                </div>
                {styleTransferServiceStatus?.detail && (
                  <div className="mt-1 text-[9px] text-amber-300/80">{styleTransferServiceStatus.detail}</div>
                )}
              </div>

              <label className="space-y-1 block">
                <span className="text-[9px] text-text-secondary/85">{t('chat.styleTransferServiceUrl')}</span>
                <div className="flex gap-1">
                  <input
                    type="text"
                    value={styleTransferServiceUrl}
                    onChange={(event) => setStyleTransferServiceUrl(event.target.value)}
                    onBlur={commitStyleTransferServiceUrl}
                    className="flex-1 bg-bg-primary rounded px-1.5 py-1 text-[10px] text-text-primary outline-none border border-surface focus:border-blue-500/50"
                  />
                  <button
                    onClick={checkStyleTransferService}
                    className="px-2 py-1 rounded bg-bg-primary text-[10px] text-text-secondary hover:text-text-primary"
                  >
                    {t('chat.retry')}
                  </button>
                </div>
              </label>

              <div className="space-y-1">
                <div className="text-[9px] text-text-secondary/85">{t('chat.styleTransferPreset')}</div>
                <div className="grid grid-cols-3 gap-1">
                  {STYLE_TRANSFER_PRESET_OPTIONS.map((option) => (
                    <button
                      key={option.value}
                      onClick={() => updateStyleTransferPreset(option.value)}
                      className={`rounded px-1.5 py-1.5 text-[10px] transition-colors ${
                        styleTransferPreset === option.value
                          ? 'bg-purple-500/20 text-purple-300'
                          : 'bg-bg-primary text-text-secondary hover:text-text-primary'
                      }`}
                    >
                      {option.label}
                    </button>
                  ))}
                </div>
              </div>

              <div className="space-y-1">
                <div className="text-[9px] text-text-secondary/85">{t('chat.styleTransferExportFormat')}</div>
                <div className="grid grid-cols-3 gap-1">
                  {STYLE_TRANSFER_EXPORT_FORMAT_OPTIONS.map((option) => (
                    <button
                      key={option.value}
                      onClick={() => updateStyleTransferExportFormat(option.value)}
                      className={`rounded px-1.5 py-1.5 text-[10px] transition-colors ${
                        styleTransferExportFormat === option.value
                          ? 'bg-purple-500/20 text-purple-300'
                          : 'bg-bg-primary text-text-secondary hover:text-text-primary'
                      }`}
                    >
                      {option.label}
                    </button>
                  ))}
                </div>
              </div>

              <label className="flex items-center justify-between gap-2 text-[10px] text-text-secondary/85">
                <span>{t('chat.styleTransferRefiner')}</span>
                <input
                  type="checkbox"
                  checked={styleTransferEnableRefiner}
                  onChange={(event) => updateStyleTransferEnableRefiner(event.target.checked)}
                  className="accent-purple-500"
                />
              </label>

              <label className="flex items-center justify-between gap-2 text-[10px] text-text-secondary/85">
                <span>{t('chat.styleTransferFallback')}</span>
                <input
                  type="checkbox"
                  checked={styleTransferAllowFallback}
                  onChange={(event) => updateStyleTransferAllowFallback(event.target.checked)}
                  className="accent-purple-500"
                />
              </label>

              <div className="rounded border border-purple-400/15 bg-purple-500/5 px-2 py-1.5 text-[9px] text-text-secondary/75">
                {t('chat.styleTransferOutputNote')}
              </div>
            </div>
          ) : (
            <div className="space-y-2">
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
          )}
        </div>
      )}
    </div>
  );
}
