import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import Text from '../ui/Text';
import { TextVariants } from '../../types/typography';
import Switch from '../ui/Switch';

interface PresetModalProps {
  isOpen: boolean;
  onClose(): void;
  onSave(name: string, includeMasks: boolean, includeCropTransform: boolean, isAdditive: boolean): void;
}

export default function AddPresetModal({ isOpen, onClose, onSave }: PresetModalProps) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [includeMasks, setIncludeMasks] = useState(false);
  const [includeCropTransform, setIncludeCropTransform] = useState(false);
  const [isAdditive, setIsAdditive] = useState(false);
  const [isMounted, setIsMounted] = useState(false);
  const [show, setShow] = useState(false);

  useEffect(() => {
    if (isOpen) {
      setIsMounted(true);
      const timer = setTimeout(() => setShow(true), 10);
      return () => clearTimeout(timer);
    } else {
      setShow(false);
      const timer = setTimeout(() => {
        setIsMounted(false);
        setName('');
        setIncludeMasks(false);
        setIncludeCropTransform(false);
        setIsAdditive(false);
      }, 300);
      return () => clearTimeout(timer);
    }
  }, [isOpen]);

  const handleSave = useCallback(() => {
    if (name.trim()) {
      onSave(name.trim(), includeMasks, includeCropTransform, isAdditive);
      onClose();
    }
  }, [name, includeMasks, includeCropTransform, isAdditive, onSave, onClose]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === 'Enter') {
        handleSave();
      } else if (e.key === 'Escape') {
        onClose();
      }
    },
    [handleSave, onClose],
  );

  if (!isMounted) {
    return null;
  }

  return (
    <div
      className={`
        fixed inset-0 flex items-center justify-center z-50
        bg-black/30 backdrop-blur-xs
        transition-opacity duration-300 ease-in-out
        ${show ? 'opacity-100' : 'opacity-0'}
      `}
      onClick={onClose}
      role="dialog"
      aria-modal="true"
    >
      <div
        className={`
          bg-surface rounded-lg shadow-xl p-6 w-full max-w-sm
          transform transition-all duration-300 ease-out
          ${show ? 'scale-100 opacity-100 translate-y-0' : 'scale-95 opacity-0 -translate-y-4'}
        `}
        onClick={(e: React.MouseEvent<HTMLDivElement>) => e.stopPropagation()}
      >
        <Text variant={TextVariants.title} className="mb-4">
          {t('modals.saveNewPreset')}
        </Text>
        <input
          autoFocus
          className="w-full bg-bg-primary text-text-primary border border-border rounded-md px-3 py-2 focus:outline-hidden focus:ring-2 focus:ring-accent"
          onChange={(e: React.ChangeEvent<HTMLInputElement>) => setName(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={t('modals.enterPresetName')}
          type="text"
          value={name}
        />

        <div className="mt-5 space-y-4 p-1">
          <Switch label="Include Masks" checked={includeMasks} onChange={setIncludeMasks} />
          <Switch label="Include Crop & Transform" checked={includeCropTransform} onChange={setIncludeCropTransform} />
          <Switch
            label="Merge Changes"
            data-tooltip="Only save changed adjustments"
            checked={isAdditive}
            onChange={setIsAdditive}
          />
        </div>

        <div className="flex justify-end gap-3 mt-6">
          <button
            className="px-4 py-2 rounded-md text-text-secondary hover:bg-surface transition-colors"
            onClick={onClose}
          >
            {t('common.cancel')}
          </button>
          <button
            className="px-4 py-2 rounded-md bg-accent text-button-text font-semibold hover:bg-accent-hover disabled:bg-gray-500 disabled:text-white disabled:cursor-not-allowed transition-colors"
            disabled={!name.trim()}
            onClick={handleSave}
          >
            {t('common.save')}
          </button>
        </div>
      </div>
    </div>
  );
}
