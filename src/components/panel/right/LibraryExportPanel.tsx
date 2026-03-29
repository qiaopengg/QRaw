import React, { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { Save, CheckCircle, XCircle, Loader, X, Ban } from 'lucide-react';
import debounce from 'lodash.debounce';
import Switch from '../../ui/Switch';
import Button from '../../ui/Button';
import Dropdown from '../../ui/Dropdown';
import Slider from '../../ui/Slider';
import ImagePicker from '../../ui/ImagePicker';
import {
  ExportPreset,
  FileFormat,
  FILE_FORMATS,
  FILENAME_VARIABLES,
  Status,
  ExportSettings,
  ExportState,
  FileFormats,
  WatermarkAnchor,
} from '../../ui/ExportImportProperties';
import { Invokes, ImageFile, AppSettings } from '../../ui/AppProperties';
import ExportPresetsList from '../../ui/ExportPresetsList';
import { useExportSettings } from '../../../hooks/useExportSettings';

interface LibraryExportPanelProps {
  exportState: ExportState;
  isVisible: boolean;
  multiSelectedPaths: Array<string>;
  onClose(): void;
  setExportState(state: ExportState): void;
  imageList: ImageFile[];
  appSettings: AppSettings | null;
  onSettingsChange: (settings: AppSettings) => void;
}

interface SectionProps {
  children: React.ReactNode;
  title: string;
}

function Section({ title, children }: SectionProps) {
  return (
    <div>
      <h3 className="text-sm font-semibold text-text-primary mb-3 border-surface pb-2">{title}</h3>
      <div className="space-y-4">{children}</div>
    </div>
  );
}

function WatermarkPreview({
  anchor,
  scale,
  spacing,
  opacity,
  watermarkPath,
  imageAspectRatio,
  watermarkImageAspectRatio,
}: {
  anchor: WatermarkAnchor;
  scale: number;
  spacing: number;
  opacity: number;
  watermarkPath: string | null;
  imageAspectRatio: number;
  watermarkImageAspectRatio: number;
}) {
  const { t } = useTranslation();
  const getPositionStyles = () => {
    const minDimPercent = imageAspectRatio > 1 ? 100 / imageAspectRatio : 100;
    const watermarkSizePercent = minDimPercent * (scale / 100);
    const spacingPercent = minDimPercent * (spacing / 100);

    const styles: React.CSSProperties = {
      width: `${watermarkSizePercent}%`,
      opacity: opacity / 100,
      position: 'absolute',
    };

    const spacingString = `${spacingPercent}%`;

    switch (anchor) {
      case WatermarkAnchor.TopLeft:
        styles.top = spacingString;
        styles.left = spacingString;
        break;
      case WatermarkAnchor.TopCenter:
        styles.top = spacingString;
        styles.left = '50%';
        styles.transform = 'translateX(-50%)';
        break;
      case WatermarkAnchor.TopRight:
        styles.top = spacingString;
        styles.right = spacingString;
        break;
      case WatermarkAnchor.CenterLeft:
        styles.top = '50%';
        styles.left = spacingString;
        styles.transform = 'translateY(-50%)';
        break;
      case WatermarkAnchor.Center:
        styles.top = '50%';
        styles.left = '50%';
        styles.transform = 'translate(-50%, -50%)';
        break;
      case WatermarkAnchor.CenterRight:
        styles.top = '50%';
        styles.right = spacingString;
        styles.transform = 'translateY(-50%)';
        break;
      case WatermarkAnchor.BottomLeft:
        styles.bottom = spacingString;
        styles.left = spacingString;
        break;
      case WatermarkAnchor.BottomCenter:
        styles.bottom = spacingString;
        styles.left = '50%';
        styles.transform = 'translateX(-50%)';
        break;
      case WatermarkAnchor.BottomRight:
        styles.bottom = spacingString;
        styles.right = spacingString;
        break;
    }
    return styles;
  };

  return (
    <div
      className="w-full bg-bg-primary rounded-md relative overflow-hidden border border-surface"
      style={{ aspectRatio: imageAspectRatio }}
    >
      <div className="absolute inset-0 flex items-center justify-center">
        <span className="text-text-tertiary text-sm">{t('export.preview')}</span>
      </div>
      {watermarkPath && (
        <div style={getPositionStyles()}>
          <div
            className="w-full bg-accent/50 border-2 border-dashed border-accent rounded-xs flex items-center justify-center"
            style={{ aspectRatio: watermarkImageAspectRatio }}
          >
            <span className="text-white text-[8px] font-bold">{t('export.logo')}</span>
          </div>
        </div>
      )}
    </div>
  );
}

const formatBytes = (bytes: number, decimals = 2) => {
  if (!+bytes) return '0 Bytes';
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(dm))} ${sizes[i]}`;
};

const getResizeModeOptions = (t: (key: string) => string) => [
  { label: t('export.longEdge'), value: 'longEdge' },
  { label: t('export.shortEdge'), value: 'shortEdge' },
  { label: t('export.width'), value: 'width' },
  { label: t('export.height'), value: 'height' },
];

export default function LibraryExportPanel({
  exportState,
  isVisible,
  multiSelectedPaths,
  onClose,
  setExportState,
  imageList: _imageList,
  appSettings,
  onSettingsChange,
}: LibraryExportPanelProps) {
  const {
    fileFormat,
    setFileFormat,
    jpegQuality,
    setJpegQuality,
    enableResize,
    setEnableResize,
    resizeMode,
    setResizeMode,
    resizeValue,
    setResizeValue,
    dontEnlarge,
    setDontEnlarge,
    keepMetadata,
    setKeepMetadata,
    stripGps,
    setStripGps,
    exportMasks,
    setExportMasks,
    filenameTemplate,
    setFilenameTemplate,
    enableWatermark,
    setEnableWatermark,
    watermarkPath,
    setWatermarkPath,
    watermarkAnchor,
    setWatermarkAnchor,
    watermarkScale,
    setWatermarkScale,
    watermarkSpacing,
    setWatermarkSpacing,
    watermarkOpacity,
    setWatermarkOpacity,
    handleApplyPreset,
    currentSettingsObject,
  } = useExportSettings();

  const { t } = useTranslation();
  const [hasLoadedSettings, setHasLoadedSettings] = useState(false);

  useEffect(() => {
    if (!isVisible) {
      setHasLoadedSettings(false);
      return;
    }

    if (appSettings && !hasLoadedSettings) {
      const lastUsed = appSettings.exportPresets?.find((p) => p.id === '__last_used__');
      if (lastUsed) {
        handleApplyPreset(lastUsed);
      }
      setHasLoadedSettings(true);
    }
  }, [isVisible, appSettings, hasLoadedSettings, handleApplyPreset]);

  const saveLastUsedPreset = useCallback(
    (exportPath: string) => {
      if (!appSettings) return;
      const lastUsedPreset: ExportPreset = {
        ...currentSettingsObject,
        id: '__last_used__',
        name: '__last_used__',
        lastExportPath: exportPath,
      };
      const updatedPresets = [
        ...(appSettings.exportPresets ?? []).filter((p) => p.id !== '__last_used__'),
        lastUsedPreset,
      ];
      onSettingsChange({ ...appSettings, exportPresets: updatedPresets });
    },
    [appSettings, currentSettingsObject, onSettingsChange],
  );

  const [estimatedSize, setEstimatedSize] = useState<number | null>(null);
  const [isEstimating, setIsEstimating] = useState<boolean>(false);
  const [watermarkImageAspectRatio, setWatermarkImageAspectRatio] = useState(1);
  const filenameInputRef = useRef<HTMLInputElement>(null);

  const { status, progress, errorMessage } = exportState;
  const isExporting = status === Status.Exporting;

  const numImages = multiSelectedPaths.length;
  const [imageAspectRatio, setImageAspectRatio] = useState(3 / 2);

  useEffect(() => {
    const fetchFirstImageDims = async () => {
      if (multiSelectedPaths.length > 0) {
        try {
          const firstPath = multiSelectedPaths[0];
          const dimensions: { width: number; height: number } = await invoke('get_image_dimensions', {
            path: firstPath,
          });
          if (dimensions.width > 0 && dimensions.height > 0) {
            setImageAspectRatio(dimensions.width / dimensions.height);
          } else {
            setImageAspectRatio(3 / 2);
          }
        } catch (_error) {
          console.warn(`Could not get dimensions for preview, using default aspect ratio.`);
          setImageAspectRatio(3 / 2);
        }
      } else {
        setImageAspectRatio(16 / 9);
      }
    };

    if (isVisible && enableWatermark) {
      fetchFirstImageDims();
    }
  }, [multiSelectedPaths, isVisible, enableWatermark]);

  useEffect(() => {
    const fetchWatermarkDimensions = async () => {
      if (watermarkPath) {
        try {
          const dimensions: { width: number; height: number } = await invoke('get_image_dimensions', {
            path: watermarkPath,
          });
          if (dimensions.height > 0) {
            setWatermarkImageAspectRatio(dimensions.width / dimensions.height);
          } else {
            setWatermarkImageAspectRatio(1);
          }
        } catch (error) {
          console.error('Failed to get watermark dimensions:', error);
          setWatermarkImageAspectRatio(1);
        }
      } else {
        setWatermarkImageAspectRatio(1);
      }
    };
    fetchWatermarkDimensions();
  }, [watermarkPath]);

  const anchorOptions = [
    { label: t('export.topLeft'), value: WatermarkAnchor.TopLeft },
    { label: t('export.topCenter'), value: WatermarkAnchor.TopCenter },
    { label: t('export.topRight'), value: WatermarkAnchor.TopRight },
    { label: t('export.centerLeft'), value: WatermarkAnchor.CenterLeft },
    { label: t('export.center'), value: WatermarkAnchor.Center },
    { label: t('export.centerRight'), value: WatermarkAnchor.CenterRight },
    { label: t('export.bottomLeft'), value: WatermarkAnchor.BottomLeft },
    { label: t('export.bottomCenter'), value: WatermarkAnchor.BottomCenter },
    { label: t('export.bottomRight'), value: WatermarkAnchor.BottomRight },
  ];

  const debouncedEstimateSize = useMemo(
    () =>
      debounce(async (paths, exportSettings, format) => {
        setIsEstimating(true);
        try {
          const size: number = await invoke(Invokes.EstimateBatchExportSize, {
            paths,
            exportSettings,
            outputFormat: format,
          });
          setEstimatedSize(size);
        } catch (err) {
          console.error('Failed to estimate batch export size:', err);
          setEstimatedSize(null);
        } finally {
          setIsEstimating(false);
        }
      }, 500),
    [],
  );

  useEffect(() => {
    if (!isVisible || multiSelectedPaths.length === 0) {
      setEstimatedSize(null);
      debouncedEstimateSize.cancel();
      return;
    }

    const exportSettings: ExportSettings = {
      filenameTemplate,
      jpegQuality,
      keepMetadata,
      resize: enableResize ? { mode: resizeMode, value: resizeValue, dontEnlarge } : null,
      stripGps,
      watermark:
        enableWatermark && watermarkPath
          ? {
              path: watermarkPath,
              anchor: watermarkAnchor,
              scale: watermarkScale,
              spacing: watermarkSpacing,
              opacity: watermarkOpacity,
            }
          : null,
      exportMasks,
    };
    const format = FILE_FORMATS.find((f: FileFormat) => f.id === fileFormat)?.extensions[0] || 'jpeg';
    debouncedEstimateSize(multiSelectedPaths, exportSettings, format);

    return () => debouncedEstimateSize.cancel();
  }, [
    isVisible,
    multiSelectedPaths,
    fileFormat,
    jpegQuality,
    enableResize,
    resizeMode,
    resizeValue,
    dontEnlarge,
    keepMetadata,
    stripGps,
    filenameTemplate,
    enableWatermark,
    watermarkPath,
    watermarkAnchor,
    watermarkScale,
    watermarkSpacing,
    watermarkOpacity,
    debouncedEstimateSize,
    exportMasks,
  ]);

  const handleVariableClick = (variable: string) => {
    if (!filenameInputRef.current) {
      return;
    }

    const input = filenameInputRef.current;
    const start = Number(input.selectionStart);
    const end = Number(input.selectionEnd);
    const currentValue = input.value;

    const newValue = currentValue.substring(0, start) + variable + currentValue.substring(end);
    setFilenameTemplate(newValue);

    setTimeout(() => {
      input.focus();
      const newCursorPos = start + variable.length;
      input.setSelectionRange(newCursorPos, newCursorPos);
    }, 0);
  };

  const handleExport = async () => {
    if (numImages === 0 || isExporting) {
      return;
    }

    let finalFilenameTemplate = filenameTemplate;
    if (
      numImages > 1 &&
      !filenameTemplate.includes('{sequence}') &&
      !filenameTemplate.includes('{original_filename}')
    ) {
      finalFilenameTemplate = `${filenameTemplate}_{sequence}`;
      setFilenameTemplate(finalFilenameTemplate);
    }

    const exportSettings: ExportSettings = {
      filenameTemplate: finalFilenameTemplate,
      jpegQuality: jpegQuality,
      keepMetadata,
      resize: enableResize ? { mode: resizeMode, value: resizeValue, dontEnlarge } : null,
      stripGps,
      exportMasks,
      watermark:
        enableWatermark && watermarkPath
          ? {
              path: watermarkPath,
              anchor: watermarkAnchor,
              scale: watermarkScale,
              spacing: watermarkSpacing,
              opacity: watermarkOpacity,
            }
          : null,
    };

    const lastExportPath = appSettings?.exportPresets?.find((p) => p.id === '__last_used__')?.lastExportPath;

    try {
      const outputFolder = await open({
        directory: true,
        title: t('export.selectFolderToExport', { count: numImages }),
        defaultPath: lastExportPath ?? undefined,
      });

      if (outputFolder) {
        saveLastUsedPreset(outputFolder as string);
        setExportState({ status: Status.Exporting, progress: { current: 0, total: numImages }, errorMessage: '' });
        await invoke(Invokes.BatchExportImages, {
          exportSettings,
          outputFolder,
          outputFormat: FILE_FORMATS.find((f: FileFormat) => f.id === fileFormat)?.extensions[0],
          paths: multiSelectedPaths,
        });
      }
    } catch (error) {
      console.error('Error exporting images:', error);
      setExportState({
        errorMessage: typeof error === 'string' ? error : 'Failed to start export.',
        progress,
        status: Status.Error,
      });
    }
  };

  const handleCancel = async () => {
    try {
      await invoke(Invokes.CancelExport);
    } catch (error) {
      console.error('Failed to send cancel request:', error);
    }
  };

  const canExport = numImages > 0;
  const isLut = fileFormat === FileFormats.Cube;
  const itemLabel = isLut ? 'LUT' : 'Image';

  return (
    <div className="h-full bg-bg-secondary rounded-lg flex flex-col">
      <div className="p-4 flex justify-between items-center shrink-0 border-b border-surface">
        <h2 className="text-xl font-bold text-primary text-shadow-shiny">Export</h2>
        <button
          onClick={onClose}
          className="p-1 rounded-md text-text-secondary hover:bg-surface hover:text-text-primary"
        >
          <X size={20} />
        </button>
      </div>
      <div className="grow overflow-y-auto p-4 text-text-secondary space-y-6">
        {canExport ? (
          <>
            <ExportPresetsList
              appSettings={appSettings}
              onSettingsChange={onSettingsChange}
              currentSettings={currentSettingsObject}
              onApplyPreset={handleApplyPreset}
            />
            <Section title={t('export.fileSettings')}>
              <div className="grid grid-cols-3 gap-2">
                {FILE_FORMATS.map((format: FileFormat) => (
                  <button
                    className={`px-2 py-1.5 text-sm rounded-md transition-colors ${
                      fileFormat === format.id ? 'bg-accent text-button-text' : 'bg-surface hover:bg-card-active'
                    } disabled:opacity-50`}
                    disabled={isExporting}
                    key={format.id}
                    onClick={() => setFileFormat(format.id)}
                  >
                    {format.name}
                  </button>
                ))}
              </div>
              {fileFormat === FileFormats.Jpeg && (
                <div className={isExporting ? 'opacity-50 pointer-events-none' : ''}>
                  <Slider
                    defaultValue={90}
                    label={t('export.quality')}
                    max={100}
                    min={1}
                    onChange={(e) => setJpegQuality(Number(e.target.value))}
                    step={1}
                    value={jpegQuality}
                  />
                </div>
              )}
            </Section>

            <Section title={t('export.fileNaming')}>
              <input
                className="w-full bg-bg-primary border border-surface rounded-md p-2 text-sm text-text-primary focus:ring-accent focus:border-accent"
                disabled={isExporting}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => setFilenameTemplate(e.target.value)}
                ref={filenameInputRef}
                type="text"
                value={filenameTemplate}
              />
              <div className="flex flex-wrap gap-2 mt-2">
                {FILENAME_VARIABLES.map((variable: string) => (
                  <button
                    className="px-2 py-1 bg-surface text-text-secondary text-xs rounded-md hover:bg-card-active transition-colors disabled:opacity-50"
                    disabled={isExporting}
                    key={variable}
                    onClick={() => handleVariableClick(variable)}
                  >
                    {variable}
                  </button>
                ))}
              </div>
            </Section>

            <Section title={t('export.imageSizing')}>
              <Switch
                label={t('export.resizeToFit')}
                checked={enableResize}
                onChange={setEnableResize}
                disabled={isExporting}
              />
              {enableResize && (
                <div className="space-y-4 pl-2 border-l-2 border-surface">
                  <div className="flex items-center gap-2">
                    <Dropdown
                      options={getResizeModeOptions(t)}
                      value={resizeMode}
                      onChange={setResizeMode}
                      disabled={isExporting}
                      className="w-full"
                    />
                    <input
                      className="w-24 bg-bg-primary text-center rounded-md p-2 border border-surface focus:border-accent focus:ring-accent"
                      disabled={isExporting}
                      min="1"
                      onChange={(e: React.ChangeEvent<HTMLInputElement>) => setResizeValue(parseInt(String(e?.target?.value)))}
                      type="number"
                      value={resizeValue}
                    />
                    <span className="text-sm">{t('export.pixels')}</span>
                  </div>
                  <Switch
                    checked={dontEnlarge}
                    disabled={isExporting}
                    label={t('export.dontEnlarge')}
                    onChange={setDontEnlarge}
                  />
                </div>
              )}
            </Section>

            <Section title={t('export.metadata')}>
              <Switch
                checked={keepMetadata}
                disabled={isExporting}
                label={t('export.keepOriginalMetadata')}
                onChange={setKeepMetadata}
              />
              {keepMetadata && (
                <div className="pl-2 border-l-2 border-surface">
                  <Switch
                    label={t('export.removeGpsData')}
                    checked={stripGps}
                    onChange={setStripGps}
                    disabled={isExporting}
                  />
                </div>
              )}
            </Section>

            <Section title={t('export.masks')}>
              <Switch
                label={t('export.exportMasksAsSeparate')}
                checked={exportMasks}
                onChange={setExportMasks}
                disabled={isExporting}
              />
            </Section>

            <Section title={t('export.watermark')}>
              <Switch
                label={t('export.addWatermark')}
                checked={enableWatermark}
                onChange={setEnableWatermark}
                disabled={isExporting}
              />
              {enableWatermark && (
                <div className="space-y-4 pl-2 border-l-2 border-surface">
                  <ImagePicker
                    label={t('export.watermarkImage')}
                    imageName={watermarkPath ? watermarkPath.split(/[\\/]/).pop() || null : null}
                    onImageSelect={setWatermarkPath}
                    onClear={() => setWatermarkPath(null)}
                  />
                  {watermarkPath && (
                    <>
                      <Dropdown
                        options={anchorOptions}
                        value={watermarkAnchor}
                        onChange={(val) => setWatermarkAnchor(val)}
                        disabled={isExporting}
                        className="w-full"
                      />
                      <Slider
                        label={t('export.scale')}
                        min={1}
                        max={50}
                        step={1}
                        value={watermarkScale}
                        onChange={(e) => setWatermarkScale(Number(e.target.value))}
                        disabled={isExporting}
                        defaultValue={10}
                      />
                      <Slider
                        label={t('export.spacing')}
                        min={0}
                        max={25}
                        step={1}
                        value={watermarkSpacing}
                        onChange={(e) => setWatermarkSpacing(Number(e.target.value))}
                        disabled={isExporting}
                        defaultValue={5}
                      />
                      <Slider
                        label={t('export.opacity')}
                        min={0}
                        max={100}
                        step={1}
                        value={watermarkOpacity}
                        onChange={(e) => setWatermarkOpacity(Number(e.target.value))}
                        disabled={isExporting}
                        defaultValue={75}
                      />
                      <WatermarkPreview
                        imageAspectRatio={imageAspectRatio}
                        watermarkImageAspectRatio={watermarkImageAspectRatio}
                        watermarkPath={watermarkPath}
                        anchor={watermarkAnchor}
                        scale={watermarkScale}
                        spacing={watermarkSpacing}
                        opacity={watermarkOpacity}
                      />
                    </>
                  )}
                </div>
              )}
            </Section>
          </>
        ) : (
          <p className="text-center text-text-tertiary mt-4">{t('libraryExport.noImagesSelected')}</p>
        )}
      </div>

      <div className="p-4 border-t border-surface shrink-0 space-y-3">
        <div className="text-center text-xs text-text-tertiary h-4">
          {isEstimating ? (
            <span className="italic">{t('export.estimatingSize')}</span>
          ) : estimatedSize !== null ? (
            <span>
              {t('export.estimatedFileSize', { size: formatBytes(estimatedSize) })}
              {numImages > 1 && ` (${formatBytes(estimatedSize / numImages)} avg)`}
            </span>
          ) : null}
        </div>
        <Button
          className={`group rounded-md h-11 w-full flex items-center text-md font-bold! justify-center ${
            status === Status.Exporting
              ? 'bg-red-600/80 hover:bg-red-600 text-white'
              : status === Status.Success
                ? 'bg-green-500/70 text-white shadow-none'
                : status === Status.Error
                  ? 'bg-red-500/20 text-red-400 shadow-none'
                  : status === Status.Cancelled
                    ? 'bg-yellow-500/20 text-yellow-400 shadow-none'
                    : ''
          }`}
          disabled={status === Status.Exporting ? false : !canExport}
          onClick={status === Status.Exporting ? handleCancel : handleExport}
          size="lg"
        >
          {status === Status.Exporting ? (
            <>
              <span className="flex items-center group-hover:hidden">
                <Loader size={18} className="animate-spin mr-2" />
                {progress.total > 1
                  ? t('export.exportingProgress', { current: progress.current, total: progress.total })
                  : t('export.exporting')}
              </span>
              <span className="hidden items-center group-hover:flex">
                <Ban size={18} className="mr-2" />
                {t('export.cancelExport')}
              </span>
            </>
          ) : status === Status.Success ? (
            <>
              <CheckCircle size={18} className="mr-2" /> {t('export.exportSuccessful')}
            </>
          ) : status === Status.Error ? (
            <>
              <XCircle size={18} className="mr-2" /> {errorMessage || t('export.exportFailed')}
            </>
          ) : status === Status.Cancelled ? (
            <>
              <Ban size={18} className="mr-2" /> {t('export.exportCancelled')}
            </>
          ) : (
            <>
              <Save size={18} className="mr-2" /> Export {numImages > 1 ? `${numImages} ${itemLabel}s` : itemLabel}
            </>
          )}
        </Button>
      </div>
    </div>
  );
}
