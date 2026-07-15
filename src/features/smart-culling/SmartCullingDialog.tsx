import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { Loader2, Sparkles } from 'lucide-react';
import Button from '../../components/ui/Button';
import Switch from '../../components/ui/Switch';
import Slider from '../../components/ui/Slider';
import Text from '../../components/ui/Text';
import { TextVariants } from '../../types/typography';
import { useUIStore } from '../../store/useUIStore';
import { SMART_CULLING_DEFAULT_SETTINGS, SMART_CULLING_INVOKES, SMART_CULLING_REVIEW_VIEW } from './constants';
import { useSmartCullingStore } from './useSmartCulling';
import { useSmartCullingEvents } from './useSmartCullingEvents';
import type { SmartCullingSettings } from './types';

interface SmartCullingDialogProps {
  imagePaths: string[];
}

/**
 * Settings-only modal. Starting an analysis closes this dialog and hands off
 * to `SmartCullingReviewPage` (a full-screen library view) once results are
 * ready, matching the agreed "header button + full-screen review workbench"
 * interaction model rather than the old single big-modal flow.
 */
export default function SmartCullingDialog({ imagePaths }: SmartCullingDialogProps) {
  const { t } = useTranslation();
  useSmartCullingEvents();
  const { dialogOpen, isRunning, progress, suggestions, error, setSmartCulling } = useSmartCullingStore();
  const setUI = useUIStore((state) => state.setUI);

  const [settings, setSettings] = useState<SmartCullingSettings>({ ...SMART_CULLING_DEFAULT_SETTINGS });
  const [isMounted, setIsMounted] = useState(false);
  const [show, setShow] = useState(false);

  useEffect(() => {
    if (dialogOpen) {
      setIsMounted(true);
      const timer = setTimeout(() => setShow(true), 10);
      return () => clearTimeout(timer);
    } else {
      setShow(false);
      const timer = setTimeout(() => setIsMounted(false), 300);
      return () => clearTimeout(timer);
    }
  }, [dialogOpen]);

  useEffect(() => {
    if (!dialogOpen || !suggestions) return;
    setSmartCulling({ dialogOpen: false });
    setUI({ activeView: SMART_CULLING_REVIEW_VIEW });
  }, [dialogOpen, suggestions, setSmartCulling, setUI]);

  const handleClose = () => setSmartCulling({ dialogOpen: false, error: null });

  const handleStart = useCallback(async () => {
    try {
      await invoke(SMART_CULLING_INVOKES.Analyze, { paths: imagePaths, settings });
    } catch (err) {
      console.error('Smart culling failed to start:', err);
      setSmartCulling({ error: String(err) });
    }
  }, [imagePaths, settings, setSmartCulling]);

  if (!isMounted) return null;

  return (
    <div
      className={`fixed inset-0 flex items-center justify-center z-50 bg-black/30 backdrop-blur-xs transition-opacity duration-300 ease-in-out ${
        show ? 'opacity-100' : 'opacity-0'
      }`}
      onClick={handleClose}
      role="dialog"
      aria-modal="true"
    >
      <div
        className={`bg-surface rounded-lg shadow-xl p-6 w-full max-w-lg transform transition-all duration-300 ease-out ${
          show ? 'scale-100 opacity-100 translate-y-0' : 'scale-95 opacity-0 -translate-y-4'
        }`}
        onClick={(e) => e.stopPropagation()}
      >
        {isRunning ? (
          <div className="flex flex-col items-center justify-center h-48">
            <Loader2 className="w-16 h-16 text-accent animate-spin" />
            <p className="mt-4 text-text-primary">{progress?.stage || t('modals.smartCulling.starting')}</p>
            {progress && progress.total > 0 && (
              <div className="w-full bg-bg-primary rounded-full h-2.5 mt-2">
                <div
                  className="bg-accent h-2.5 rounded-full"
                  style={{ width: `${(progress.current / progress.total) * 100}%` }}
                />
              </div>
            )}
          </div>
        ) : (
          <>
            <div className="flex items-center justify-center mb-4">
              <Sparkles className="w-12 h-12 text-accent" />
            </div>
            <Text variant={TextVariants.title} className="mb-6 text-center">
              {t('modals.smartCulling.title')}
            </Text>
            {error && (
              <Text as="div" className="mb-4 text-red-500">
                {error}
              </Text>
            )}
            <div className="space-y-6 text-sm">
              <div>
                <Switch
                  label={t('modals.smartCulling.groupSimilar')}
                  checked={settings.groupSimilar}
                  onChange={(v) => setSettings((s) => ({ ...s, groupSimilar: v }))}
                />
                {settings.groupSimilar && (
                  <div className="mt-2 pl-4 border-l-2 border-border-color ml-1">
                    <Slider
                      label={t('modals.smartCulling.similarityThreshold')}
                      min={1}
                      max={64}
                      step={1}
                      value={settings.similarityThreshold}
                      defaultValue={28}
                      onChange={(e) => setSettings((s) => ({ ...s, similarityThreshold: Number(e.target.value) }))}
                      fillOrigin="min"
                    />
                    <Text variant={TextVariants.small} className="mt-1">
                      {t('modals.smartCulling.similarityThresholdDesc')}
                    </Text>
                  </div>
                )}
              </div>
              <div>
                <Switch
                  label={t('modals.smartCulling.filterBlurry')}
                  checked={settings.filterBlurry}
                  onChange={(v) => setSettings((s) => ({ ...s, filterBlurry: v }))}
                />
                {settings.filterBlurry && (
                  <div className="mt-2  pl-4 border-l-2 border-border-color ml-1">
                    <Slider
                      label={t('modals.smartCulling.blurThreshold')}
                      min={25}
                      max={500}
                      step={25}
                      value={settings.blurThreshold}
                      defaultValue={100.0}
                      onChange={(e) => setSettings((s) => ({ ...s, blurThreshold: Number(e.target.value) }))}
                      fillOrigin="min"
                    />
                    <Text variant={TextVariants.small} className="mt-1">
                      {t('modals.smartCulling.blurThresholdDesc')}
                    </Text>
                  </div>
                )}
              </div>
              <div>
                <Switch
                  label={t('modals.smartCulling.detectFaces')}
                  checked={settings.detectFaces}
                  onChange={(v) => setSettings((s) => ({ ...s, detectFaces: v }))}
                />
                {settings.detectFaces && (
                  <div className="mt-2 pl-4 border-l-2 border-border-color ml-1">
                    <Text variant={TextVariants.small} className="mt-1">
                      {t('modals.smartCulling.detectFacesDesc')}
                    </Text>
                  </div>
                )}
              </div>
            </div>
            <div className="flex justify-end gap-3 mt-8">
              <button
                className="px-4 py-2 rounded-md text-text-secondary hover:bg-surface transition-colors"
                onClick={handleClose}
              >
                {t('modals.smartCulling.cancel')}
              </button>
              <Button onClick={handleStart} disabled={imagePaths.length === 0}>
                {t('modals.smartCulling.startCulling')}
              </Button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
