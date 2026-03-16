import React, { useEffect, useState } from 'react';
import {
  ArrowLeft,
  Cloud,
  Cpu,
  ExternalLink as ExternalLinkIcon,
  Server,
  Info,
  Trash2,
  Wifi,
  WifiOff,
  Plus,
  X,
  SlidersHorizontal,
  Keyboard,
  Bookmark,
  Scaling,
  Image as ImageIcon,
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { relaunch } from '@tauri-apps/plugin-process';
import { motion, AnimatePresence } from 'framer-motion';
import clsx from 'clsx';
import { useUser } from '@clerk/clerk-react';
import { useTranslation } from 'react-i18next';
import i18n, { LANGUAGES } from '../../i18n';
import Button from '../ui/Button';
import ConfirmModal from '../modals/ConfirmModal';
import Dropdown, { OptionItem } from '../ui/Dropdown';
import Switch from '../ui/Switch';
import Input from '../ui/Input';
import Slider from '../ui/Slider';
import { ThemeProps, THEMES, DEFAULT_THEME_ID } from '../../utils/themes';
import { Invokes, AppSettings } from '../ui/AppProperties';
import Text from '../ui/Text';
import { TextColors, TextVariants, TextWeights } from '../../types/typography';
import { platform } from '@tauri-apps/plugin-os';

interface ConfirmModalState {
  confirmText: string;
  confirmVariant: string;
  isOpen: boolean;
  message: string;
  onConfirm(): void;
  title: string;
}

interface DataActionItemProps {
  buttonAction(): void;
  buttonText: string;
  description: React.ReactNode;
  disabled?: boolean;
  icon: React.ReactNode;
  isProcessing: boolean;
  message: string;
  title: string;
}

interface KeybindItemProps {
  description: string;
  keys: Array<string>;
}

interface SettingItemProps {
  children: React.ReactNode;
  description?: string;
  label: string;
}

interface SettingsPanelProps {
  appSettings: AppSettings;
  onBack(): void;
  onLibraryRefresh(): void;
  onSettingsChange(settings: AppSettings): void;
  rootPath: string | null;
}

interface TestStatus {
  message: string;
  success: boolean | null;
  testing: boolean;
}

interface MyLens {
  maker: string;
  model: string;
}

const EXECUTE_TIMEOUT = 3000;

const adjustmentVisibilityDefaults = {
  sharpening: true,
  presence: true,
  noiseReduction: true,
  chromaticAberration: false,
  vignette: true,
  colorCalibration: false,
  grain: true,
};

const resolutions: OptionItem<number>[] = [
  { value: 720, label: '720px' },
  { value: 1280, label: '1280px' },
  { value: 1920, label: '1920px' },
  { value: 2560, label: '2560px' },
  { value: 3840, label: '3840px' },
];

const zoomMultiplierOptions: OptionItem<number>[] = [
  { value: 1.0, label: '1.0x (Native)' },
  { value: 0.75, label: '0.75x' },
  { value: 0.5, label: '0.50x (Half)' },
  { value: 0.25, label: '0.25x' },
];

const backendOptions = (t: (key: string) => string): OptionItem<string>[] => [
  { value: 'auto', label: t('settings.backend_auto') },
  { value: 'vulkan', label: t('settings.backend_vulkan') },
  { value: 'dx12', label: t('settings.backend_dx12') },
  { value: 'metal', label: t('settings.backend_metal') },
  { value: 'gl', label: t('settings.backend_gl') },
];

const linearRawOptions = (t: (key: string) => string): OptionItem<string>[] => [
  { value: 'auto', label: t('settings.linearRaw_auto') },
  { value: 'gamma', label: t('settings.linearRaw_gamma') },
  { value: 'skip_calib', label: t('settings.linearRaw_skip_calib') },
  { value: 'gamma_skip_calib', label: t('settings.linearRaw_gamma_skip_calib') },
];

const settingCategories = (t: (key: string) => string) => [
  { id: 'general', label: t('settings.general'), icon: SlidersHorizontal },
  { id: 'processing', label: t('settings.processing'), icon: Cpu },
  { id: 'shortcuts', label: t('settings.shortcuts'), icon: Keyboard },
];

const KeybindItem = ({ keys, description }: KeybindItemProps) => (
  <div className="flex justify-between items-center py-2">
    <Text variant={TextVariants.label}>{description}</Text>
    <div className="flex items-center gap-1">
      {keys.map((key: string, index: number) => (
        <Text
          as="kbd"
          variant={TextVariants.small}
          color={TextColors.primary}
          weight={TextWeights.semibold}
          key={index}
          className="px-2 py-1 font-sans bg-bg-primary border border-border-color rounded-md"
        >
          {key}
        </Text>
      ))}
    </div>
  </div>
);

const SettingItem = ({ children, description, label }: SettingItemProps) => (
  <div>
    <Text variant={TextVariants.heading} className="block mb-2">
      {label}
    </Text>
    {children}
    {description && (
      <Text variant={TextVariants.small} className="mt-2">
        {description}
      </Text>
    )}
  </div>
);

const DataActionItem = ({
  buttonAction,
  buttonText,
  description,
  disabled = false,
  icon,
  isProcessing,
  message,
  title,
}: DataActionItemProps) => {
  const { t } = useTranslation();
  return (
    <div className="pb-8 border-b border-border-color last:border-b-0 last:pb-0">
      <Text variant={TextVariants.heading} className="mb-2">
        {title}
      </Text>
      <Text variant={TextVariants.small} className="mb-3">
        {description}
      </Text>
      <Button variant="destructive" onClick={buttonAction} disabled={isProcessing || disabled}>
        {icon}
        {isProcessing ? t('common.processing') : buttonText}
      </Button>
      {message && (
        <Text color={TextColors.accent} className="mt-3">
          {message}
        </Text>
      )}
    </div>
  );
};

interface AiProviderSwitchProps {
  selectedProvider: string;
  onProviderChange: (provider: string) => void;
}

const AiProviderSwitch = ({ selectedProvider, onProviderChange }: AiProviderSwitchProps) => {
  const { t } = useTranslation();
  const aiProviders = [
    { id: 'cpu', label: 'CPU', icon: Cpu },
    { id: 'ai-connector', label: t('settings.selfHosted').split(' ')[0], icon: Server },
    { id: 'cloud', label: t('settings.cloudService'), icon: Cloud },
  ];
  return (
    <div className="relative flex w-full p-1 bg-bg-primary rounded-md border border-border-color">
      {aiProviders.map((provider) => (
        <button
          key={provider.id}
          onClick={() => onProviderChange(provider.id)}
          className={clsx(
            'relative flex-1 flex items-center justify-center gap-2 px-3 py-1.5 text-sm font-medium rounded-md transition-colors',
            {
              'text-text-primary hover:bg-surface': selectedProvider !== provider.id,
              'text-button-text': selectedProvider === provider.id,
            },
          )}
          style={{ WebkitTapHighlightColor: 'transparent' }}
        >
          {selectedProvider === provider.id && (
            <motion.span
              layoutId="ai-provider-switch-bubble"
              className="absolute inset-0 z-0 bg-accent"
              style={{ borderRadius: 6 }}
              transition={{ type: 'spring', bounce: 0.2, duration: 0.6 }}
            />
          )}
          <span className="relative z-10 flex items-center">
            <provider.icon size={16} className="mr-2" />
            {provider.label}
          </span>
        </button>
      ))}
    </div>
  );
};

interface PreviewModeSwitchProps {
  mode: 'static' | 'dynamic';
  onModeChange: (mode: 'static' | 'dynamic') => void;
}

const PreviewModeSwitch = ({ mode, onModeChange }: PreviewModeSwitchProps) => {
  const { t } = useTranslation();
  const previewModes = [
    { id: 'static', label: t('settings.fixedResolution'), icon: ImageIcon },
    { id: 'dynamic', label: t('settings.dynamic'), icon: Scaling },
  ];
  return (
    <div className="relative flex w-full p-1 bg-bg-primary rounded-md border border-border-color">
      {previewModes.map((item) => (
        <button
          key={item.id}
          onClick={() => onModeChange(item.id as 'static' | 'dynamic')}
          className={clsx(
            'relative flex-1 flex items-center justify-center gap-2 px-3 py-1.5 text-sm font-medium rounded-md transition-colors',
            {
              'text-text-primary hover:bg-surface': mode !== item.id,
              'text-button-text': mode === item.id,
            },
          )}
          style={{ WebkitTapHighlightColor: 'transparent' }}
        >
          {mode === item.id && (
            <motion.span
              layoutId="preview-mode-switch-bubble"
              className="absolute inset-0 z-0 bg-accent"
              style={{ borderRadius: 6 }}
              transition={{ type: 'spring', bounce: 0.2, duration: 0.6 }}
            />
          )}
          <span className="relative z-10 flex items-center">
            <item.icon size={16} className="mr-2" />
            {item.label}
          </span>
        </button>
      ))}
    </div>
  );
};

export default function SettingsPanel({
  appSettings,
  onBack,
  onLibraryRefresh,
  onSettingsChange,
  rootPath,
}: SettingsPanelProps) {
  const { user: _user } = useUser();
  const { t } = useTranslation();
  const [isClearing, setIsClearing] = useState(false);
  const [clearMessage, setClearMessage] = useState('');
  const [isClearingCache, setIsClearingCache] = useState(false);
  const [cacheClearMessage, setCacheClearMessage] = useState('');
  const [isClearingAiTags, setIsClearingAiTags] = useState(false);
  const [aiTagsClearMessage, setAiTagsClearMessage] = useState('');
  const [isClearingTags, setIsClearingTags] = useState(false);
  const [tagsClearMessage, setTagsClearMessage] = useState('');
  const [confirmModalState, setConfirmModalState] = useState<ConfirmModalState>({
    confirmText: t('common.confirm'),
    confirmVariant: 'primary',
    isOpen: false,
    message: '',
    onConfirm: () => {},
    title: '',
  });
  const [testStatus, setTestStatus] = useState<TestStatus>({ message: '', success: null, testing: false });
  const [hasInteractedWithLivePreview, setHasInteractedWithLivePreview] = useState(false);

  const [aiProvider, setAiProvider] = useState(appSettings?.aiProvider || 'cpu');
  const [aiConnectorAddress, setAiConnectorAddress] = useState<string>(appSettings?.aiConnectorAddress || '');
  const [llmEndpoint, setLlmEndpoint] = useState<string>(appSettings?.llmEndpoint || 'http://localhost:11434');
  const [llmApiKey, setLlmApiKey] = useState<string>(appSettings?.llmApiKey || '');
  const [llmModel, setLlmModel] = useState<string>(appSettings?.llmModel || 'qwen3.5:9b');
  const [newShortcut, setNewShortcut] = useState('');
  const [newAiTag, setNewAiTag] = useState('');

  const [lensMakers, setLensMakers] = useState<string[]>([]);
  const [lensModels, setLensModels] = useState<string[]>([]);
  const [tempLensMaker, setTempLensMaker] = useState<string>('');
  const [tempLensModel, setTempLensModel] = useState<string>('');

  const [processingSettings, setProcessingSettings] = useState({
    editorPreviewResolution: appSettings?.editorPreviewResolution || 1920,
    rawHighlightCompression: appSettings?.rawHighlightCompression ?? 2.5,
    processingBackend: appSettings?.processingBackend || 'auto',
    linuxGpuOptimization: appSettings?.linuxGpuOptimization ?? false,
    highResZoomMultiplier: appSettings?.highResZoomMultiplier || 1.0,
    useFullDpiRendering: appSettings?.useFullDpiRendering ?? false,
  });
  const [restartRequired, setRestartRequired] = useState(false);
  const [activeCategory, setActiveCategory] = useState('general');
  const [logPath, setLogPath] = useState('');
  const [dpr, setDpr] = useState(() => (typeof window !== 'undefined' ? window.devicePixelRatio : 1));
  const [osPlatform, setOsPlatform] = useState('');

  useEffect(() => {
    try {
      setOsPlatform(platform());
    } catch (e) {
      console.error('Failed to get platform:', e);
    }
  }, []);

  const filteredBackendOptions = backendOptions(t).filter((opt) => {
    if (opt.value === 'metal' && osPlatform !== 'macos') return false;
    if (opt.value === 'dx12' && osPlatform === 'macos') return false;
    return true;
  });

  useEffect(() => {
    if (typeof window === 'undefined') return;

    const updateDpr = () => setDpr(window.devicePixelRatio);

    const mediaQuery = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
    mediaQuery.addEventListener('change', updateDpr);

    window.addEventListener('resize', updateDpr);

    return () => {
      mediaQuery.removeEventListener('change', updateDpr);
      window.removeEventListener('resize', updateDpr);
    };
  }, []);

  const customAiTags = Array.from(new Set<string>(appSettings?.customAiTags || []));
  const taggingShortcuts = Array.from(new Set<string>(appSettings?.taggingShortcuts || []));

  useEffect(() => {
    if (appSettings?.aiConnectorAddress !== aiConnectorAddress) {
      setAiConnectorAddress(appSettings?.aiConnectorAddress || '');
    }
    if (appSettings?.aiProvider !== aiProvider) {
      setAiProvider(appSettings?.aiProvider || 'cpu');
    }
    setProcessingSettings({
      editorPreviewResolution: appSettings?.editorPreviewResolution || 1920,
      rawHighlightCompression: appSettings?.rawHighlightCompression ?? 2.5,
      processingBackend: appSettings?.processingBackend || 'auto',
      linuxGpuOptimization: appSettings?.linuxGpuOptimization ?? false,
      highResZoomMultiplier: appSettings?.highResZoomMultiplier || 1.0,
      useFullDpiRendering: appSettings?.useFullDpiRendering ?? false,
    });
    setRestartRequired(false);
  }, [appSettings]);

  useEffect(() => {
    const fetchLogPath = async () => {
      try {
        const path: string = await invoke(Invokes.GetLogFilePath);
        setLogPath(path);
      } catch (error) {
        console.error('Failed to get log file path:', error);
        setLogPath('Could not retrieve log file path.');
      }
    };
    fetchLogPath();

    invoke('get_lensfun_makers')
      .then((m: unknown) => setLensMakers(m as string[]))
      .catch(console.error);
  }, []);

  const handleProcessingSettingChange = (key: string, value: unknown) => {
    setProcessingSettings((prev) => ({ ...prev, [key]: value }));
    if (key === 'processingBackend' || key === 'linuxGpuOptimization') {
      setRestartRequired(true);
    } else {
      onSettingsChange({ ...appSettings, [key]: value });
    }
  };

  const handleSaveAndRelaunch = async () => {
    onSettingsChange({
      ...appSettings,
      ...processingSettings,
    });
    await new Promise((resolve) => setTimeout(resolve, 200));
    await relaunch();
  };

  const handleProviderChange = (provider: string) => {
    setAiProvider(provider);
    onSettingsChange({ ...appSettings, aiProvider: provider });
  };

  const handlePreviewModeChange = (mode: 'static' | 'dynamic') => {
    const enableZoomHifi = mode === 'dynamic';
    onSettingsChange({ ...appSettings, enableZoomHifi });
  };

  const handleTempMakerChange = (maker: string) => {
    setTempLensMaker(maker);
    setTempLensModel('');
    setLensModels([]);
    if (maker) {
      invoke('get_lensfun_lenses_for_maker', { maker })
        .then((l: unknown) => setLensModels(l as string[]))
        .catch(console.error);
    }
  };

  const handleAddLens = () => {
    if (tempLensMaker && tempLensModel) {
      const currentLenses: MyLens[] = appSettings?.myLenses || [];
      if (!currentLenses.some((l) => l.maker === tempLensMaker && l.model === tempLensModel)) {
        const newLenses = [...currentLenses, { maker: tempLensMaker, model: tempLensModel }];

        newLenses.sort((a, b) => {
          const makerComp = a.maker.localeCompare(b.maker);
          if (makerComp !== 0) return makerComp;
          return a.model.localeCompare(b.model);
        });

        onSettingsChange({
          ...appSettings,
          myLenses: newLenses,
        });
        setTempLensMaker('');
        setTempLensModel('');
        setLensModels([]);
      }
    }
  };

  const handleRemoveLens = (index: number) => {
    const currentLenses: MyLens[] = appSettings?.myLenses || [];
    const newLenses = [...currentLenses];
    newLenses.splice(index, 1);
    onSettingsChange({ ...appSettings, myLenses: newLenses });
  };

  const effectiveRootPath = rootPath || appSettings?.lastRootPath;

  const executeClearSidecars = async () => {
    setIsClearing(true);
    setClearMessage(t('settings.deletingSidecars'));
    try {
      const count: number = await invoke(Invokes.ClearAllSidecars, { rootPath: effectiveRootPath });
      setClearMessage(t('settings.sidecarsDeleted', { count }));
      onLibraryRefresh();
    } catch (err: unknown) {
      console.error('Failed to clear sidecars:', err);
      setClearMessage(`Error: ${err}`);
    } finally {
      setTimeout(() => {
        setIsClearing(false);
        setClearMessage('');
      }, EXECUTE_TIMEOUT);
    }
  };

  const handleClearSidecars = () => {
    setConfirmModalState({
      confirmText: t('settings.deleteAllEdits'),
      confirmVariant: 'destructive',
      isOpen: true,
      message: t('settings.confirmDeletionMessage'),
      onConfirm: executeClearSidecars,
      title: t('settings.confirmDeletion'),
    });
  };

  const executeClearAiTags = async () => {
    setIsClearingAiTags(true);
    setAiTagsClearMessage(t('settings.clearingAiTags'));
    try {
      const count: number = await invoke(Invokes.ClearAiTags, { rootPath: effectiveRootPath });
      setAiTagsClearMessage(t('settings.aiTagsCleared', { count }));
      onLibraryRefresh();
    } catch (err: unknown) {
      console.error('Failed to clear AI tags:', err);
      setAiTagsClearMessage(`Error: ${err}`);
    } finally {
      setTimeout(() => {
        setIsClearingAiTags(false);
        setAiTagsClearMessage('');
      }, EXECUTE_TIMEOUT);
    }
  };

  const handleClearAiTags = () => {
    setConfirmModalState({
      confirmText: t('settings.clearAiTags'),
      confirmVariant: 'destructive',
      isOpen: true,
      message: t('settings.confirmAiTagDeletionMessage'),
      onConfirm: executeClearAiTags,
      title: t('settings.confirmAiTagDeletion'),
    });
  };

  const executeClearTags = async () => {
    setIsClearingTags(true);
    setTagsClearMessage(t('settings.clearingAllTags'));
    try {
      const count: number = await invoke(Invokes.ClearAllTags, { rootPath: effectiveRootPath });
      setTagsClearMessage(t('settings.allTagsCleared', { count }));
      onLibraryRefresh();
    } catch (err: unknown) {
      console.error('Failed to clear tags:', err);
      setTagsClearMessage(`Error: ${err}`);
    } finally {
      setTimeout(() => {
        setIsClearingTags(false);
        setTagsClearMessage('');
      }, EXECUTE_TIMEOUT);
    }
  };

  const handleClearTags = () => {
    setConfirmModalState({
      confirmText: t('settings.clearAllTags'),
      confirmVariant: 'destructive',
      isOpen: true,
      message: t('settings.confirmAllTagDeletionMessage'),
      onConfirm: executeClearTags,
      title: t('settings.confirmAllTagDeletion'),
    });
  };

  const shortcutTagVariants = {
    visible: { opacity: 1, scale: 1, transition: { type: 'spring' as const, stiffness: 500, damping: 30 } },
    exit: { opacity: 0, scale: 0.8, transition: { duration: 0.15 } },
  };

  const executeSetTransparent = async (transparent: boolean) => {
    onSettingsChange({ ...appSettings, transparent });
    await relaunch();
  };

  const handleSetTransparent = (transparent: boolean) => {
    setConfirmModalState({
      confirmText: t('settings.toggleTransparency'),
      confirmVariant: 'primary',
      isOpen: true,
      message: transparent
        ? t('settings.confirmWindowTransparencyMessage_enable')
        : t('settings.confirmWindowTransparencyMessage_disable'),
      onConfirm: () => executeSetTransparent(transparent),
      title: t('settings.confirmWindowTransparency'),
    });
  };

  const executeClearCache = async () => {
    setIsClearingCache(true);
    setCacheClearMessage(t('settings.clearingThumbnailCache'));
    try {
      await invoke(Invokes.ClearThumbnailCache);
      setCacheClearMessage(t('settings.thumbnailCacheCleared'));
      onLibraryRefresh();
    } catch (err: unknown) {
      console.error('Failed to clear thumbnail cache:', err);
      setCacheClearMessage(`Error: ${err}`);
    } finally {
      setTimeout(() => {
        setIsClearingCache(false);
        setCacheClearMessage('');
      }, EXECUTE_TIMEOUT);
    }
  };

  const handleClearCache = () => {
    setConfirmModalState({
      confirmText: t('settings.clearCache'),
      confirmVariant: 'destructive',
      isOpen: true,
      message: t('settings.confirmCacheDeletionMessage'),
      onConfirm: executeClearCache,
      title: t('settings.confirmCacheDeletion'),
    });
  };

  const handleTestConnection = async () => {
    if (!aiConnectorAddress) {
      return;
    }
    setTestStatus({ testing: true, message: t('common.testing'), success: null });
    try {
      await invoke(Invokes.TestAIConnectorConnection, { address: aiConnectorAddress });
      setTestStatus({ testing: false, message: t('settings.connectionSuccessful'), success: true });
    } catch (err) {
      setTestStatus({ testing: false, message: t('settings.connectionFailed'), success: false });
      console.error('AI Connector connection test failed:', err);
    } finally {
      setTimeout(() => setTestStatus({ testing: false, message: '', success: null }), EXECUTE_TIMEOUT);
    }
  };

  const closeConfirmModal = () => {
    setConfirmModalState({ ...confirmModalState, isOpen: false });
  };

  const handleAddShortcut = () => {
    const parsedTags = newShortcut
      .split(',')
      .map((t) => t.trim().toLowerCase())
      .filter((t) => t.length > 0);

    if (parsedTags.length > 0) {
      const uniqueShortcuts = Array.from(new Set([...taggingShortcuts, ...parsedTags])).sort();
      onSettingsChange({ ...appSettings, taggingShortcuts: uniqueShortcuts });
    }
    setNewShortcut('');
  };

  const handleRemoveShortcut = (shortcutToRemove: string) => {
    const uniqueShortcuts = taggingShortcuts.filter((s) => s !== shortcutToRemove);
    onSettingsChange({ ...appSettings, taggingShortcuts: uniqueShortcuts });
  };

  const handleInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleAddShortcut();
    }
  };

  const handleAddAiTag = () => {
    const parsedTags = newAiTag
      .split(',')
      .map((t) => t.trim().toLowerCase())
      .filter((t) => t.length > 0);

    if (parsedTags.length > 0) {
      const uniqueTags = Array.from(new Set([...customAiTags, ...parsedTags])).sort();
      onSettingsChange({ ...appSettings, customAiTags: uniqueTags });
    }
    setNewAiTag('');
  };

  const handleRemoveAiTag = (tagToRemove: string) => {
    const uniqueTags = customAiTags.filter((t) => t !== tagToRemove);
    onSettingsChange({ ...appSettings, customAiTags: uniqueTags });
  };

  const handleAiTagInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleAddAiTag();
    }
  };

  return (
    <>
      <ConfirmModal {...confirmModalState} onClose={closeConfirmModal} />
      <div className="flex flex-col h-full w-full text-text-primary">
        <header className="flex-shrink-0 flex flex-wrap items-center justify-between gap-y-4 mb-8 pt-4">
          <div className="flex items-center flex-shrink-0">
            <Button
              className="mr-4 hover:bg-surface text-text-primary rounded-full"
              onClick={onBack}
              size="icon"
              variant="ghost"
              data-tooltip={t('common.goToHome')}
            >
              <ArrowLeft />
            </Button>
            <Text variant={TextVariants.display} color={TextColors.accent} className="whitespace-nowrap">
              {t('settings.title')}
            </Text>
          </div>

          <div className="relative flex w-full min-[1200px]:w-[450px] p-2 bg-surface rounded-md">
            {settingCategories(t).map((category) => (
              <button
                key={category.id}
                onClick={() => setActiveCategory(category.id)}
                className={clsx(
                  'relative flex-1 flex items-center justify-center gap-2 px-3 py-1.5 text-sm font-medium rounded-md transition-colors',
                  {
                    'text-text-primary hover:bg-surface': activeCategory !== category.id,
                    'text-button-text': activeCategory === category.id,
                  },
                )}
                style={{ WebkitTapHighlightColor: 'transparent' }}
              >
                {activeCategory === category.id && (
                  <motion.span
                    layoutId="settings-category-switch-bubble"
                    className="absolute inset-0 z-0 bg-accent"
                    style={{ borderRadius: 6 }}
                    transition={{ type: 'spring', bounce: 0.2, duration: 0.6 }}
                  />
                )}
                <span className="relative z-10 flex items-center">
                  <category.icon size={16} className="mr-2 flex-shrink-0" />
                  <span className="truncate">{category.label}</span>
                </span>
              </button>
            ))}
          </div>
        </header>

        <div className="flex-1 overflow-y-auto overflow-x-hidden pr-2 -mr-2 custom-scrollbar">
          <AnimatePresence mode="wait">
            {activeCategory === 'general' && (
              <motion.div
                key="general"
                initial={{ opacity: 0, x: 10 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: -10 }}
                transition={{ duration: 0.2 }}
                className="space-y-10"
              >
                <div className="p-6 bg-surface rounded-xl shadow-md">
                  <Text variant={TextVariants.title} color={TextColors.accent} className="mb-8">
                    {t('settings.generalSettings')}
                  </Text>
                  <div className="space-y-8">
                    <SettingItem label={t('settings.theme')} description={t('settings.themeDescription')}>
                      <Dropdown
                        onChange={(value) => onSettingsChange({ ...appSettings, theme: value })}
                        options={THEMES.map((theme: ThemeProps) => ({
                          value: theme.id,
                          label: t(`settings.theme_${theme.id}`, { defaultValue: theme.name }),
                        }))}
                        value={appSettings?.theme || DEFAULT_THEME_ID}
                      />
                    </SettingItem>

                    <SettingItem
                      description={t('settings.adaptiveEditorThemeDescription')}
                      label={t('settings.editorTheme')}
                    >
                      <Switch
                        checked={appSettings?.adaptiveEditorTheme ?? false}
                        id="adaptive-theme-toggle"
                        label={t('settings.adaptiveEditorTheme')}
                        onChange={(checked) => onSettingsChange({ ...appSettings, adaptiveEditorTheme: checked })}
                      />
                    </SettingItem>

                    <SettingItem
                      label={t('settings.exifLibrarySorting')}
                      description={t('settings.exifLibrarySortingDescription')}
                    >
                      <Switch
                        checked={appSettings?.enableExifReading ?? false}
                        id="exif-reading-toggle"
                        label={t('settings.exifReading')}
                        onChange={(checked) => onSettingsChange({ ...appSettings, enableExifReading: checked })}
                      />
                    </SettingItem>

                    <SettingItem
                      label={t('settings.xmpMetadataSync')}
                      description={t('settings.xmpMetadataSyncDescription')}
                    >
                      <Switch
                        checked={appSettings?.enableXmpSync ?? true}
                        id="enable-xmp-sync-toggle"
                        label={t('settings.enableXmpSync')}
                        onChange={(checked) => {
                          const newSettings = { ...appSettings, enableXmpSync: checked };
                          if (!checked) {
                            newSettings.createXmpIfMissing = false;
                          }
                          onSettingsChange(newSettings);
                        }}
                      />
                    </SettingItem>

                    <SettingItem
                      label={t('settings.createMissingXmpFiles')}
                      description={t('settings.createMissingXmpFilesDescription')}
                    >
                      <Switch
                        disabled={!appSettings?.enableXmpSync}
                        checked={appSettings?.createXmpIfMissing ?? false}
                        id="create-xmp-missing-toggle"
                        label={t('settings.createXmpIfMissing')}
                        onChange={(checked) => onSettingsChange({ ...appSettings, createXmpIfMissing: checked })}
                      />
                    </SettingItem>

                    <SettingItem
                      label={t('settings.folderImageCounts')}
                      description={t('settings.folderImageCountsDescription')}
                    >
                      <Switch
                        checked={appSettings?.enableFolderImageCounts ?? false}
                        id="folder-image-counts-toggle"
                        label={t('settings.showImageCounts')}
                        onChange={(checked) => onSettingsChange({ ...appSettings, enableFolderImageCounts: checked })}
                      />
                    </SettingItem>

                    <SettingItem
                      description={t('settings.windowEffectsDescription')}
                      label={t('settings.windowEffects')}
                    >
                      <Switch
                        checked={appSettings?.transparent ?? true}
                        id="window-effects-toggle"
                        label={t('settings.transparency')}
                        onChange={handleSetTransparent}
                      />
                    </SettingItem>

                    <SettingItem label={t('settings.font')} description={t('settings.fontDescription')}>
                      <Dropdown
                        onChange={(value) => onSettingsChange({ ...appSettings, fontFamily: value })}
                        options={[
                          { value: 'poppins', label: t('settings.font_poppins') },
                          { value: 'system', label: t('settings.font_system') },
                        ]}
                        value={appSettings?.fontFamily || 'poppins'}
                      />
                    </SettingItem>

                    <SettingItem label={t('settings.language')} description={t('settings.languageDescription')}>
                      <Dropdown
                        onChange={(value) => i18n.changeLanguage(value)}
                        options={LANGUAGES.map((lang) => ({ value: lang.code, label: lang.label }))}
                        value={i18n.language}
                      />
                    </SettingItem>
                  </div>
                </div>

                <div className="p-6 bg-surface rounded-xl shadow-md">
                  <Text variant={TextVariants.title} color={TextColors.accent} className="mb-8">
                    {t('settings.adjustmentsVisibility')}
                  </Text>
                  <Text className="mb-4">{t('settings.adjustmentsVisibilityDescription')}</Text>
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-4">
                    <Switch
                      label={t('adjustments.chromaticAberration')}
                      checked={appSettings?.adjustmentVisibility?.chromaticAberration ?? false}
                      onChange={(checked) =>
                        onSettingsChange({
                          ...appSettings,
                          adjustmentVisibility: {
                            ...(appSettings?.adjustmentVisibility || adjustmentVisibilityDefaults),
                            chromaticAberration: checked,
                          },
                        })
                      }
                    />
                    <Switch
                      label={t('adjustments.grain')}
                      checked={appSettings?.adjustmentVisibility?.grain ?? true}
                      onChange={(checked) =>
                        onSettingsChange({
                          ...appSettings,
                          adjustmentVisibility: {
                            ...(appSettings?.adjustmentVisibility || adjustmentVisibilityDefaults),
                            grain: checked,
                          },
                        })
                      }
                    />
                    <Switch
                      label={t('adjustments.colorCalibration')}
                      checked={appSettings?.adjustmentVisibility?.colorCalibration ?? true}
                      onChange={(checked) =>
                        onSettingsChange({
                          ...appSettings,
                          adjustmentVisibility: {
                            ...(appSettings?.adjustmentVisibility || adjustmentVisibilityDefaults),
                            colorCalibration: checked,
                          },
                        })
                      }
                    />
                  </div>
                </div>

                <div className="p-6 bg-surface rounded-xl shadow-md">
                  <Text variant={TextVariants.title} color={TextColors.accent} className="mb-8">
                    {t('settings.myLenses')}
                  </Text>
                  <Text className="mb-6">{t('settings.myLensesDescription')}</Text>

                  <div className="space-y-8">
                    <div className="bg-bg-primary rounded-lg p-4 border border-border-color">
                      <Text variant={TextVariants.heading} className="mb-3">
                        {t('settings.addNewLens')}
                      </Text>
                      <div className="space-y-4">
                        <Dropdown
                          options={lensMakers.map((m) => ({ label: m, value: m }))}
                          value={tempLensMaker}
                          onChange={handleTempMakerChange}
                          placeholder={t('settings.selectManufacturer')}
                        />
                        <Dropdown
                          options={lensModels.map((m) => ({ label: m, value: m }))}
                          value={tempLensModel}
                          onChange={setTempLensModel}
                          placeholder={t('settings.selectLensModel')}
                          disabled={!tempLensMaker}
                        />
                        <Button onClick={handleAddLens} disabled={!tempLensMaker || !tempLensModel} className="w-full">
                          <Plus size={16} className="mr-1" />
                          {t('settings.addToMyLenses')}
                        </Button>
                      </div>
                    </div>

                    <div>
                      <Text variant={TextVariants.heading} className="mb-2">
                        {t('settings.savedLenses')}
                      </Text>
                      {(!appSettings?.myLenses || appSettings.myLenses.length === 0) && (
                        <Text className="italic">{t('settings.noLensesAdded')}</Text>
                      )}
                      <div className="divide-y divide-border-color">
                        {(appSettings?.myLenses || []).map((lens: MyLens, index: number) => (
                          <div
                            key={`${lens.maker}-${lens.model}-${index}`}
                            className="flex justify-between items-center py-3 first:pt-0 last:pb-0"
                          >
                            <div className="flex items-center gap-3">
                              <div className="p-2 bg-surface rounded-md text-accent">
                                <Bookmark size={16} />
                              </div>
                              <div>
                                <Text color={TextColors.primary} weight={TextWeights.medium}>
                                  {lens.model}
                                </Text>
                                <Text variant={TextVariants.small}>{lens.maker}</Text>
                              </div>
                            </div>
                            <button
                              onClick={() => handleRemoveLens(index)}
                              className="p-2 text-text-secondary hover:text-red-400 hover:bg-bg-primary rounded-md transition-colors"
                              data-tooltip={t('settings.removeLens')}
                            >
                              <Trash2 size={16} />
                            </button>
                          </div>
                        ))}
                      </div>
                    </div>
                  </div>
                </div>

                <div className="p-6 bg-surface rounded-xl shadow-md">
                  <Text variant={TextVariants.title} color={TextColors.accent} className="mb-8">
                    {t('settings.tagging')}
                  </Text>
                  <div className="space-y-8">
                    <div className="space-y-4">
                      <SettingItem description={t('settings.aiTaggingDescription')} label={t('settings.aiTagging')}>
                        <Switch
                          checked={appSettings?.enableAiTagging ?? false}
                          id="ai-tagging-toggle"
                          label={t('settings.automaticAiTagging')}
                          onChange={(checked) => onSettingsChange({ ...appSettings, enableAiTagging: checked })}
                        />
                      </SettingItem>

                      <AnimatePresence>
                        {(appSettings?.enableAiTagging ?? false) && (
                          <motion.div
                            initial={{ height: 0, opacity: 0 }}
                            animate={{ height: 'auto', opacity: 1 }}
                            exit={{ height: 0, opacity: 0 }}
                            transition={{ duration: 0.3, ease: 'easeInOut' }}
                            className="overflow-hidden"
                          >
                            <div className="pl-4 border-l-2 border-border-color ml-1 space-y-8">
                              <SettingItem
                                label={t('settings.maximumAiTags')}
                                description={t('settings.maximumAiTagsDescription')}
                              >
                                <Slider
                                  label={t('adjustments.amount')}
                                  min={1}
                                  max={20}
                                  step={1}
                                  value={appSettings?.aiTagCount ?? 10}
                                  defaultValue={10}
                                  onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                                    onSettingsChange({ ...appSettings, aiTagCount: parseInt(e.target.value) })
                                  }
                                />
                              </SettingItem>

                              <SettingItem
                                label={t('settings.customAiTagList')}
                                description={t('settings.customAiTagListDescription')}
                              >
                                <div>
                                  <div className="flex flex-wrap gap-2 p-2 bg-bg-primary rounded-md min-h-[40px] border border-border-color mb-2 items-center">
                                    <AnimatePresence>
                                      {customAiTags.length > 0 ? (
                                        customAiTags.map((tag: string) => (
                                          <motion.div
                                            key={tag}
                                            layout
                                            variants={shortcutTagVariants}
                                            initial={false}
                                            animate="visible"
                                            exit="exit"
                                            onClick={() => handleRemoveAiTag(tag)}
                                            data-tooltip={t('settings.removeTag', { tag })}
                                            className="flex items-center gap-1 bg-surface px-2 py-1 rounded group cursor-pointer"
                                          >
                                            <Text variant={TextVariants.label} color={TextColors.primary}>
                                              {tag}
                                            </Text>
                                            <span className="rounded-full group-hover:bg-black/20 p-0.5 transition-colors">
                                              <X size={14} />
                                            </span>
                                          </motion.div>
                                        ))
                                      ) : (
                                        <motion.span
                                          key="no-ai-tags-placeholder"
                                          initial={{ opacity: 0 }}
                                          animate={{ opacity: 1 }}
                                          exit={{ opacity: 0 }}
                                          transition={{ duration: 0.2 }}
                                        >
                                          <Text className="px-1 select-none italic">
                                            {t('settings.noCustomAiTags')}
                                          </Text>
                                        </motion.span>
                                      )}
                                    </AnimatePresence>
                                  </div>
                                  <div className="flex items-center gap-2">
                                    <div className="relative flex-1">
                                      <Input
                                        type="text"
                                        value={newAiTag}
                                        onChange={(e) => setNewAiTag(e.target.value)}
                                        onKeyDown={handleAiTagInputKeyDown}
                                        placeholder="{t('settings.addCustomAiTags')}"
                                        className="pr-10"
                                      />
                                      <button
                                        onClick={handleAddAiTag}
                                        className="absolute right-1 top-1/2 -translate-y-1/2 p-1.5 rounded-full text-text-secondary hover:text-text-primary hover:bg-surface"
                                        data-tooltip={t('settings.addAiTag')}
                                      >
                                        <Plus size={18} />
                                      </button>
                                    </div>
                                    <button
                                      onClick={() => onSettingsChange({ ...appSettings, customAiTags: [] })}
                                      disabled={customAiTags.length === 0}
                                      className="p-2 text-text-secondary hover:text-red-400 hover:bg-surface rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:text-text-secondary disabled:hover:bg-transparent"
                                      data-tooltip={t('settings.clearAiTagList')}
                                    >
                                      <Trash2 size={18} />
                                    </button>
                                  </div>
                                </div>
                              </SettingItem>
                            </div>
                          </motion.div>
                        )}
                      </AnimatePresence>
                    </div>

                    <SettingItem
                      label={t('settings.taggingShortcuts')}
                      description={t('settings.taggingShortcutsDescription')}
                    >
                      <div>
                        <div className="flex flex-wrap gap-2 p-2 bg-bg-primary rounded-md min-h-[40px] border border-border-color mb-2 items-center">
                          <AnimatePresence>
                            {taggingShortcuts.length > 0 ? (
                              taggingShortcuts.map((shortcut: string) => (
                                <motion.div
                                  key={shortcut}
                                  layout
                                  variants={shortcutTagVariants}
                                  initial={false}
                                  animate="visible"
                                  exit="exit"
                                  onClick={() => handleRemoveShortcut(shortcut)}
                                  data-tooltip={t('settings.removeShortcut', { shortcut })}
                                  className="flex items-center gap-1 bg-surface px-2 py-1 rounded group cursor-pointer"
                                >
                                  <Text variant={TextVariants.label} color={TextColors.primary}>
                                    {shortcut}
                                  </Text>
                                  <span className="rounded-full group-hover:bg-black/20 p-0.5 transition-colors">
                                    <X size={14} />
                                  </span>
                                </motion.div>
                              ))
                            ) : (
                              <motion.span
                                key="no-shortcuts-placeholder"
                                initial={{ opacity: 0 }}
                                animate={{ opacity: 1 }}
                                exit={{ opacity: 0 }}
                                transition={{ duration: 0.2 }}
                                className="text-sm text-text-secondary italic px-1 select-none"
                              >
                                {t('settings.noShortcutsAdded')}
                              </motion.span>
                            )}
                          </AnimatePresence>
                        </div>
                        <div className="flex items-center gap-2">
                          <div className="relative flex-1">
                            <Input
                              type="text"
                              value={newShortcut}
                              onChange={(e) => setNewShortcut(e.target.value)}
                              onKeyDown={handleInputKeyDown}
                              placeholder="{t('settings.addShortcuts')}"
                              className="pr-10"
                            />
                            <button
                              onClick={handleAddShortcut}
                              className="absolute right-1 top-1/2 -translate-y-1/2 p-1.5 rounded-full text-text-secondary hover:text-text-primary hover:bg-surface"
                              data-tooltip={t('settings.addShortcut')}
                            >
                              <Plus size={18} />
                            </button>
                          </div>
                          <button
                            onClick={() => onSettingsChange({ ...appSettings, taggingShortcuts: [] })}
                            disabled={taggingShortcuts.length === 0}
                            className="p-2 text-text-secondary hover:text-red-400 hover:bg-surface rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:text-text-secondary disabled:hover:bg-transparent"
                            data-tooltip={t('settings.clearShortcutsTagList')}
                          >
                            <Trash2 size={18} />
                          </button>
                        </div>
                      </div>
                    </SettingItem>

                    <div className="pt-8 border-t border-border-color">
                      <div className="space-y-8">
                        <DataActionItem
                          buttonAction={handleClearAiTags}
                          buttonText={t('common.clear')}
                          description={t('settings.clearAiTagsDescription')}
                          disabled={!effectiveRootPath}
                          icon={<Trash2 size={16} className="mr-2" />}
                          isProcessing={isClearingAiTags}
                          message={aiTagsClearMessage}
                          title={t('settings.clearAiTags')}
                        />
                        <DataActionItem
                          buttonAction={handleClearTags}
                          buttonText={t('common.clear')}
                          description={t('settings.clearAllTagsDescription')}
                          disabled={!effectiveRootPath}
                          icon={<Trash2 size={16} className="mr-2" />}
                          isProcessing={isClearingTags}
                          message={tagsClearMessage}
                          title={t('settings.clearAllTags')}
                        />
                      </div>
                    </div>
                  </div>
                </div>
              </motion.div>
            )}

            {activeCategory === 'processing' && (
              <motion.div
                key="processing"
                initial={{ opacity: 0, x: 10 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: -10 }}
                transition={{ duration: 0.2 }}
                className="space-y-10"
              >
                <div className="p-6 bg-surface rounded-xl shadow-md">
                  <Text variant={TextVariants.title} color={TextColors.accent} className="mb-8">
                    {t('settings.processingEngine')}
                  </Text>
                  <div className="space-y-8">
                    <div>
                      <Text variant={TextVariants.heading} className="mb-2">
                        {t('settings.previewRenderingStrategy')}
                      </Text>
                      <PreviewModeSwitch
                        mode={appSettings?.enableZoomHifi ? 'dynamic' : 'static'}
                        onModeChange={handlePreviewModeChange}
                      />

                      <div className="mt-3">
                        <AnimatePresence mode="wait">
                          {!(appSettings?.enableZoomHifi ?? false) ? (
                            <motion.div
                              key="static-preview"
                              initial={{ opacity: 0, x: 10 }}
                              animate={{ opacity: 1, x: 0 }}
                              exit={{ opacity: 0, x: -10 }}
                              transition={{ duration: 0.2 }}
                            >
                              <Text variant={TextVariants.small} className="mb-4">
                                {t('settings.staticPreviewDescription')}
                              </Text>
                              <div className="pl-4 border-l-2 border-border-color ml-1">
                                <SettingItem
                                  description={t('settings.previewResolutionDescription')}
                                  label={t('settings.previewResolution')}
                                >
                                  <Dropdown
                                    onChange={(value) =>
                                      handleProcessingSettingChange('editorPreviewResolution', value)
                                    }
                                    options={resolutions}
                                    value={processingSettings.editorPreviewResolution}
                                  />
                                </SettingItem>
                              </div>
                            </motion.div>
                          ) : (
                            <motion.div
                              key="dynamic-preview"
                              initial={{ opacity: 0, x: 10 }}
                              animate={{ opacity: 1, x: 0 }}
                              exit={{ opacity: 0, x: -10 }}
                              transition={{ duration: 0.2 }}
                            >
                              <Text variant={TextVariants.small} className="mb-4">
                                {t('settings.dynamicPreviewDescription')}
                              </Text>
                              <div className="pl-4 border-l-2 border-border-color ml-1 space-y-3">
                                <SettingItem
                                  description={t('settings.staticPreviewResolutionDescription')}
                                  label={t('settings.staticPreviewResolution')}
                                >
                                  <Dropdown
                                    onChange={(value) =>
                                      handleProcessingSettingChange('editorPreviewResolution', value)
                                    }
                                    options={resolutions}
                                    value={processingSettings.editorPreviewResolution}
                                  />
                                </SettingItem>

                                <SettingItem
                                  label={t('settings.renderResolutionScale')}
                                  description={t('settings.renderResolutionScaleDescription')}
                                >
                                  <Dropdown
                                    onChange={(value) => handleProcessingSettingChange('highResZoomMultiplier', value)}
                                    options={zoomMultiplierOptions}
                                    value={processingSettings.highResZoomMultiplier}
                                  />
                                </SettingItem>

                                <SettingItem
                                  label={t('settings.highDpiRendering')}
                                  description={
                                    dpr > 1
                                      ? t('settings.highDpiRenderingDescriptionActive', { dpr })
                                      : t('settings.highDpiRenderingDescriptionInactive')
                                  }
                                >
                                  <Switch
                                    checked={processingSettings.useFullDpiRendering}
                                    disabled={dpr <= 1}
                                    id="full-dpi-rendering-toggle"
                                    label={t('settings.renderAtNativeDpi')}
                                    onChange={(checked) =>
                                      handleProcessingSettingChange('useFullDpiRendering', checked)
                                    }
                                  />
                                </SettingItem>
                              </div>
                            </motion.div>
                          )}
                        </AnimatePresence>
                      </div>
                    </div>

                    <div className="space-y-4">
                      <SettingItem
                        label={t('settings.liveInteractivePreviews')}
                        description={t('settings.liveInteractivePreviewsDescription')}
                      >
                        <Switch
                          checked={appSettings?.enableLivePreviews ?? true}
                          id="live-previews-toggle"
                          label={t('settings.enableLivePreviews')}
                          onChange={(checked) => {
                            setHasInteractedWithLivePreview(true);
                            onSettingsChange({ ...appSettings, enableLivePreviews: checked });
                          }}
                        />
                      </SettingItem>

                      <AnimatePresence>
                        {(appSettings?.enableLivePreviews ?? true) && (
                          <motion.div
                            initial={hasInteractedWithLivePreview ? { height: 0, opacity: 0 } : false}
                            animate={{ height: 'auto', opacity: 1 }}
                            exit={{ height: 0, opacity: 0 }}
                            transition={{ duration: 0.3, ease: 'easeInOut' }}
                            className="overflow-hidden"
                          >
                            <div className="pl-4 border-l-2 border-border-color ml-1">
                              <SettingItem
                                label={t('settings.highQualityLivePreview')}
                                description={t('settings.highQualityLivePreviewDescription')}
                              >
                                <Switch
                                  checked={appSettings?.enableHighQualityLivePreviews ?? false}
                                  id="hq-live-previews-toggle"
                                  label={t('settings.enableHighQuality')}
                                  onChange={(checked) =>
                                    onSettingsChange({ ...appSettings, enableHighQualityLivePreviews: checked })
                                  }
                                />
                              </SettingItem>
                            </div>
                          </motion.div>
                        )}
                      </AnimatePresence>
                    </div>

                    <SettingItem
                      label={t('settings.rawHighlightRecovery')}
                      description={t('settings.rawHighlightRecoveryDescription')}
                    >
                      <Slider
                        label={t('adjustments.amount')}
                        min={1}
                        max={10}
                        step={0.1}
                        value={processingSettings.rawHighlightCompression}
                        defaultValue={2.5}
                        onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                          handleProcessingSettingChange('rawHighlightCompression', parseFloat(e.target.value))
                        }
                      />
                    </SettingItem>

                    <SettingItem
                      label={t('settings.linearRawProcessing')}
                      description={t('settings.linearRawProcessingDescription')}
                    >
                      <Dropdown
                        onChange={(value) => onSettingsChange({ ...appSettings, linearRawMode: value })}
                        options={linearRawOptions(t)}
                        value={appSettings?.linearRawMode || 'auto'}
                      />
                    </SettingItem>

                    <SettingItem
                      label={t('settings.processingBackend')}
                      description={t('settings.processingBackendDescription')}
                    >
                      <Dropdown
                        onChange={(value) => handleProcessingSettingChange('processingBackend', value)}
                        options={filteredBackendOptions}
                        value={
                          filteredBackendOptions.some((option) => option.value === processingSettings.processingBackend)
                            ? processingSettings.processingBackend
                            : 'auto'
                        }
                      />
                    </SettingItem>

                    {osPlatform !== 'macos' && osPlatform !== 'windows' && (
                      <SettingItem
                        label={t('settings.linuxCompatibilityMode')}
                        description={t('settings.linuxCompatibilityModeDescription')}
                      >
                        <Switch
                          checked={processingSettings.linuxGpuOptimization}
                          id="gpu-compat-toggle"
                          label={t('settings.enableCompatibilityMode')}
                          onChange={(checked) => handleProcessingSettingChange('linuxGpuOptimization', checked)}
                        />
                      </SettingItem>
                    )}

                    {restartRequired && (
                      <>
                        <Text
                          as="div"
                          color={TextColors.info}
                          className="p-3 bg-blue-900/10 border border-blue-500/50 rounded-lg flex items-center gap-3"
                        >
                          <Info size={18} />
                          <p>{t('settings.restartRequired')}</p>
                        </Text>
                        <div className="flex justify-end">
                          <Button onClick={handleSaveAndRelaunch}>{t('common.saveAndRelaunch')}</Button>
                        </div>
                      </>
                    )}
                  </div>
                </div>

                <div className="p-6 bg-surface rounded-xl shadow-md">
                  <Text variant={TextVariants.title} color={TextColors.accent} className="mb-8">
                    {t('settings.generativeAi')}
                  </Text>
                  <Text className="mb-4">{t('settings.generativeAiDescription')}</Text>

                  <AiProviderSwitch selectedProvider={aiProvider} onProviderChange={handleProviderChange} />

                  <div className="mt-8">
                    <AnimatePresence mode="wait">
                      {aiProvider === 'cpu' && (
                        <motion.div
                          key="cpu"
                          initial={{ opacity: 0, x: 10 }}
                          animate={{ opacity: 1, x: 0 }}
                          exit={{ opacity: 0, x: -10 }}
                          transition={{ duration: 0.2 }}
                        >
                          <Text variant={TextVariants.heading}>{t('settings.builtInAiCpu')}</Text>
                          <Text className="mt-1">{t('settings.builtInAiCpuDescription')}</Text>
                          <Text as="ul" className="mt-3 space-y-1 list-disc list-inside">
                            <li>{t('settings.aiMaskingFeatures')}</li>
                            <li>{t('settings.automaticImageTagging')}</li>
                            <li>{t('settings.simpleCpuGenerativeReplace')}</li>
                          </Text>
                        </motion.div>
                      )}

                      {aiProvider === 'ai-connector' && (
                        <motion.div
                          key="ai-connector"
                          initial={{ opacity: 0, x: 10 }}
                          animate={{ opacity: 1, x: 0 }}
                          exit={{ opacity: 0, x: -10 }}
                          transition={{ duration: 0.2 }}
                        >
                          <div className="space-y-8">
                            <div>
                              <Text variant={TextVariants.heading}>{t('settings.selfHosted')}</Text>
                              <Text className="mt-1">{t('settings.selfHostedDescription')}</Text>
                              <Text as="ul" className="mt-3 space-y-1 list-disc list-inside">
                                <li>{t('settings.useOwnComfyUI')}</li>
                                <li>{t('settings.costFreeAdvancedEdits')}</li>
                                <li>{t('settings.customWorkflowSelection')}</li>
                              </Text>
                            </div>
                            <SettingItem
                              label={t('settings.aiConnectorAddress')}
                              description={t('settings.aiConnectorAddressDescription')}
                            >
                              <div className="flex items-center gap-2">
                                <Input
                                  className="flex-grow"
                                  id="ai-connector-address"
                                  onBlur={() =>
                                    onSettingsChange({ ...appSettings, aiConnectorAddress: aiConnectorAddress })
                                  }
                                  onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                                    setAiConnectorAddress(e.target.value)
                                  }
                                  onKeyDown={(e: React.KeyboardEvent<HTMLInputElement>) => e.stopPropagation()}
                                  placeholder="127.0.0.1:8188"
                                  type="text"
                                  value={aiConnectorAddress}
                                />
                                <Button
                                  className="w-32"
                                  disabled={testStatus.testing || !aiConnectorAddress}
                                  onClick={handleTestConnection}
                                >
                                  {testStatus.testing ? t('common.testing') : t('common.test')}
                                </Button>
                              </div>
                              {testStatus.message && (
                                <Text
                                  color={testStatus.success ? TextColors.success : TextColors.error}
                                  className="mt-2 flex items-center gap-2"
                                >
                                  {testStatus.success === true && <Wifi size={16} />}
                                  {testStatus.success === false && <WifiOff size={16} />}
                                  {testStatus.message}
                                </Text>
                              )}
                            </SettingItem>
                          </div>
                        </motion.div>
                      )}

                      {aiProvider === 'cloud' && (
                        <motion.div
                          key="cloud"
                          initial={{ opacity: 0, x: 10 }}
                          animate={{ opacity: 1, x: 0 }}
                          exit={{ opacity: 0, x: -10 }}
                          transition={{ duration: 0.2 }}
                        >
                          <Text variant={TextVariants.heading}>{t('settings.cloudService')}</Text>
                          <Text className="mt-1">{t('settings.cloudServiceDescription')}</Text>
                          <Text as="ul" className="mt-3 space-y-1 list-disc list-inside">
                            <li>{t('settings.maxConvenience')}</li>
                            <li>{t('settings.sameResultsSelfHosting')}</li>
                            <li>{t('settings.noPowerfulHardware')}</li>
                          </Text>

                          <div className="mt-8 p-4 bg-bg-primary rounded-lg border border-border-color text-center space-y-3">
                            <Text
                              variant={TextVariants.small}
                              color={TextColors.button}
                              weight={TextWeights.semibold}
                              className="inline-block bg-accent px-2 py-1 rounded-full"
                            >
                              {t('settings.comingSoon')}
                            </Text>
                            <Text>{t('settings.comingSoonDescription')}</Text>
                          </div>
                        </motion.div>
                      )}
                    </AnimatePresence>
                  </div>
                </div>

                <div className="p-6 bg-surface rounded-xl shadow-md">
                  <Text variant={TextVariants.title} color={TextColors.accent} className="mb-8">
                    {t('chat.llmSettings')}
                  </Text>
                  <div className="space-y-6">
                    <SettingItem label={t('chat.llmEndpoint')} description="Ollama 默认地址：http://localhost:11434">
                      <Input
                        value={llmEndpoint}
                        onChange={(e: React.ChangeEvent<HTMLInputElement>) => setLlmEndpoint(e.target.value)}
                        onBlur={() => onSettingsChange({ ...appSettings, llmEndpoint })}
                        onKeyDown={(e: React.KeyboardEvent<HTMLInputElement>) => e.stopPropagation()}
                        placeholder={t('chat.llmEndpointPlaceholder')}
                        type="text"
                      />
                    </SettingItem>
                    <SettingItem
                      label={t('chat.llmModel')}
                      description="推荐：qwen2.5:7b（中文）或 llama3.2:3b（英文）"
                    >
                      <Input
                        value={llmModel}
                        onChange={(e: React.ChangeEvent<HTMLInputElement>) => setLlmModel(e.target.value)}
                        onBlur={() => onSettingsChange({ ...appSettings, llmModel })}
                        onKeyDown={(e: React.KeyboardEvent<HTMLInputElement>) => e.stopPropagation()}
                        placeholder={t('chat.llmModelPlaceholder')}
                        type="text"
                      />
                    </SettingItem>
                    <SettingItem label={t('chat.llmApiKey')} description="使用 OpenAI 等付费服务时填写">
                      <Input
                        value={llmApiKey}
                        onChange={(e: React.ChangeEvent<HTMLInputElement>) => setLlmApiKey(e.target.value)}
                        onBlur={() => onSettingsChange({ ...appSettings, llmApiKey })}
                        onKeyDown={(e: React.KeyboardEvent<HTMLInputElement>) => e.stopPropagation()}
                        placeholder={t('chat.llmApiKeyPlaceholder')}
                        type="password"
                      />
                    </SettingItem>
                  </div>
                </div>

                <div className="p-6 bg-surface rounded-xl shadow-md">
                  <Text variant={TextVariants.title} color={TextColors.accent} className="mb-8">
                    {t('settings.dataManagement')}
                  </Text>
                  <div className="space-y-8">
                    <DataActionItem
                      buttonAction={handleClearSidecars}
                      buttonText={t('common.clear')}
                      description={
                        <Text as="span" variant={TextVariants.small}>
                          This will delete all{' '}
                          <code className="bg-bg-primary px-1 rounded text-text-primary">.qcr</code> files
                          (containing your edits) within the current base folder:
                          <span className="block font-mono bg-bg-primary p-2 rounded mt-2 break-all border border-border-color">
                            {effectiveRootPath || t('settings.noFolderSelected')}
                          </span>
                        </Text>
                      }
                      disabled={!effectiveRootPath}
                      icon={<Trash2 size={16} className="mr-2" />}
                      isProcessing={isClearing}
                      message={clearMessage}
                      title={t('settings.clearAllSidecarFiles')}
                    />

                    <DataActionItem
                      buttonAction={handleClearCache}
                      buttonText={t('common.clear')}
                      description={t('settings.clearThumbnailCacheDescription')}
                      icon={<Trash2 size={16} className="mr-2" />}
                      isProcessing={isClearingCache}
                      message={cacheClearMessage}
                      title={t('settings.clearThumbnailCache')}
                    />

                    <DataActionItem
                      buttonAction={async () => {
                        if (logPath && !logPath.startsWith('Could not')) {
                          await invoke(Invokes.ShowInFinder, { path: logPath });
                        }
                      }}
                      buttonText={t('common.open')}
                      description={
                        <Text as="span" variant={TextVariants.small}>
                          {t('settings.viewApplicationLogsDescription')}
                          <span className="block font-mono bg-bg-primary p-2 rounded mt-2 break-all border border-border-color">
                            {logPath || t('common.loading')}
                          </span>
                        </Text>
                      }
                      disabled={!logPath || logPath.startsWith('Could not')}
                      icon={<ExternalLinkIcon size={16} className="mr-2" />}
                      isProcessing={false}
                      message=""
                      title={t('settings.viewApplicationLogs')}
                    />
                  </div>
                </div>
              </motion.div>
            )}

            {activeCategory === 'shortcuts' && (
              <motion.div
                key="shortcuts"
                initial={{ opacity: 0, x: 10 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: -10 }}
                transition={{ duration: 0.2 }}
                className="space-y-10"
              >
                <div className="p-6 bg-surface rounded-xl shadow-md">
                  <Text variant={TextVariants.title} color={TextColors.accent} className="mb-8">
                    {t('settings.keyboardShortcuts')}
                  </Text>
                  <div className="space-y-8">
                    <div>
                      <Text variant={TextVariants.heading}>{t('settings.generalShortcuts')}</Text>
                      <div className="divide-y divide-border-color">
                        <KeybindItem keys={['Space', 'Enter']} description={t('settings.shortcut_openSelectedImage')} />
                        <KeybindItem
                          keys={['Ctrl/Cmd', '+', 'C']}
                          description={t('settings.shortcut_copyAdjustments')}
                        />
                        <KeybindItem
                          keys={['Ctrl/Cmd', '+', 'V']}
                          description={t('settings.shortcut_pasteAdjustments')}
                        />
                        <KeybindItem
                          keys={['Ctrl/Cmd', '+', 'Shift', '+', 'C']}
                          description={t('settings.shortcut_copyFiles')}
                        />
                        <KeybindItem
                          description={t('settings.shortcut_pasteFiles')}
                          keys={['Ctrl/Cmd', '+', 'Shift', '+', 'V']}
                        />
                        <KeybindItem keys={['Ctrl/Cmd', '+', 'A']} description={t('settings.shortcut_selectAll')} />
                        <KeybindItem keys={['Delete']} description={t('settings.shortcut_deleteFiles')} />
                        <KeybindItem keys={['0-5']} description={t('settings.shortcut_setRating')} />
                        <KeybindItem keys={['Shift', '+', '0-5']} description={t('settings.shortcut_setColorLabel')} />
                        <KeybindItem keys={['↑', '↓', '←', '→']} description={t('settings.shortcut_navigateImages')} />
                      </div>
                    </div>
                    <div>
                      <Text variant={TextVariants.heading}>{t('settings.editorShortcuts')}</Text>
                      <div className="divide-y divide-border-color">
                        <KeybindItem keys={['Esc']} description={t('settings.shortcut_deselectExit')} />
                        <KeybindItem keys={['Ctrl/Cmd', '+', 'Z']} description={t('settings.shortcut_undo')} />
                        <KeybindItem keys={['Ctrl/Cmd', '+', 'Y']} description={t('settings.shortcut_redo')} />
                        <KeybindItem keys={['Delete']} description={t('settings.shortcut_deleteMask')} />
                        <KeybindItem keys={['Space']} description={t('settings.shortcut_cycleZoom')} />
                        <KeybindItem keys={['←', '→']} description={t('settings.shortcut_prevNextImage')} />
                        <KeybindItem keys={['↑', '↓']} description={t('settings.shortcut_zoomInOut')} />
                        <KeybindItem
                          keys={['Shift', '+', 'Mouse Wheel']}
                          description={t('settings.shortcut_adjustSlider')}
                        />
                        <KeybindItem keys={['Ctrl/Cmd', '+', '+']} description={t('settings.shortcut_zoomIn')} />
                        <KeybindItem keys={['Ctrl/Cmd', '+', '-']} description={t('settings.shortcut_zoomOut')} />
                        <KeybindItem keys={['Ctrl/Cmd', '+', '0']} description={t('settings.shortcut_zoomToFit')} />
                        <KeybindItem keys={['Ctrl/Cmd', '+', '1']} description={t('settings.shortcut_zoomTo100')} />
                        <KeybindItem keys={['F']} description={t('settings.shortcut_toggleFullscreen')} />
                        <KeybindItem keys={['B']} description={t('settings.shortcut_showOriginal')} />
                        <KeybindItem keys={['D']} description={t('settings.shortcut_toggleAdjustments')} />
                        <KeybindItem keys={['R']} description={t('settings.shortcut_toggleCrop')} />
                        <KeybindItem keys={['M']} description={t('settings.shortcut_toggleMasks')} />
                        <KeybindItem keys={['K']} description={t('settings.shortcut_toggleAi')} />
                        <KeybindItem keys={['P']} description={t('settings.shortcut_togglePresets')} />
                        <KeybindItem keys={['I']} description={t('settings.shortcut_toggleMetadata')} />
                        <KeybindItem keys={['W']} description={t('settings.shortcut_toggleWaveform')} />
                        <KeybindItem keys={['E']} description={t('settings.shortcut_toggleExport')} />
                      </div>
                    </div>
                  </div>
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </div>
    </>
  );
}
