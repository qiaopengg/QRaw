import React from 'react';
import { RotateCcw, Copy, ClipboardPaste, Aperture } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import BasicAdjustments from '../../adjustments/Basic';
import CurveGraph from '../../adjustments/Curves';
import ColorPanel from '../../adjustments/Color';
import DetailsPanel from '../../adjustments/Details';
import EffectsPanel from '../../adjustments/Effects';
import CollapsibleSection from '../../ui/CollapsibleSection';
import { Adjustments, SectionVisibility, INITIAL_ADJUSTMENTS, ADJUSTMENT_SECTIONS } from '../../../utils/adjustments';
import { useContextMenu } from '../../../context/ContextMenuContext';
import { OPTION_SEPARATOR, SelectedImage, AppSettings, CollapsibleSectionsState } from '../../ui/AppProperties';
import { ChannelConfig } from '../../adjustments/Curves';

interface ControlsPanelOption {
  disabled?: boolean;
  icon?: React.ComponentType<{ size?: number; className?: string }>;
  label?: string;
  onClick?(): void;
  type?: string;
}

interface CopiedSectionAdjustments {
  section: string;
  values: Partial<Adjustments>;
}

interface ControlsProps {
  adjustments: Adjustments;
  collapsibleState: CollapsibleSectionsState;
  copiedSectionAdjustments: CopiedSectionAdjustments | null;
  handleAutoAdjustments(): void;
  handleLutSelect(path: string): void;
  histogram: ChannelConfig | null;
  selectedImage: SelectedImage;
  setAdjustments(adjustments: Adjustments | ((prev: Adjustments) => Adjustments)): void;
  setCollapsibleState(
    state: CollapsibleSectionsState | ((prev: CollapsibleSectionsState) => CollapsibleSectionsState),
  ): void;
  setCopiedSectionAdjustments(adjustments: CopiedSectionAdjustments): void;
  theme: string;
  appSettings: AppSettings | null;
  isWbPickerActive?: boolean;
  toggleWbPicker?: () => void;
  onDragStateChange?: (isDragging: boolean) => void;
}

export default function Controls({
  adjustments,
  collapsibleState,
  copiedSectionAdjustments,
  handleAutoAdjustments,
  handleLutSelect,
  histogram,
  selectedImage,
  setAdjustments,
  setCollapsibleState,
  setCopiedSectionAdjustments,
  theme,
  appSettings,
  isWbPickerActive,
  toggleWbPicker,
  onDragStateChange,
}: ControlsProps) {
  const { t } = useTranslation();
  const { showContextMenu } = useContextMenu();

  const handleToggleVisibility = (sectionName: string) => {
    setAdjustments((prev: Adjustments) => {
      const currentVisibility: SectionVisibility = prev.sectionVisibility || INITIAL_ADJUSTMENTS.sectionVisibility;
      return {
        ...prev,
        sectionVisibility: {
          ...currentVisibility,
          [sectionName]: !currentVisibility[sectionName],
        },
      };
    });
  };

  const handleResetAdjustments = () => {
    setAdjustments((prev: Adjustments) => ({
      ...prev,
      ...Object.keys(ADJUSTMENT_SECTIONS)
        .flatMap((s) => ADJUSTMENT_SECTIONS[s])
        .reduce((acc: Record<string, unknown>, key: string) => {
          acc[key] = INITIAL_ADJUSTMENTS[key];
          return acc;
        }, {}),
      sectionVisibility: { ...INITIAL_ADJUSTMENTS.sectionVisibility },
    }));
  };

  const handleToggleSection = (section: string) => {
    setCollapsibleState((prev: CollapsibleSectionsState) => ({ ...prev, [section]: !prev[section] }));
  };

  const handleSectionContextMenu = (event: React.MouseEvent, sectionName: string) => {
    event.preventDefault();
    event.stopPropagation();

    const sectionKeys = ADJUSTMENT_SECTIONS[sectionName];
    if (!sectionKeys) {
      return;
    }

    const handleCopy = () => {
      const adjustmentsToCopy: Partial<Adjustments> = {};
      for (const key of sectionKeys) {
        if (Object.prototype.hasOwnProperty.call(adjustments, key)) {
          (adjustmentsToCopy as Record<string, unknown>)[key] = JSON.parse(JSON.stringify(adjustments[key]));
        }
      }
      setCopiedSectionAdjustments({ section: sectionName, values: adjustmentsToCopy });
    };

    const handlePaste = () => {
      if (!copiedSectionAdjustments || copiedSectionAdjustments.section !== sectionName) {
        return;
      }
      setAdjustments((prev: Adjustments) => ({
        ...prev,
        ...copiedSectionAdjustments.values,
        sectionVisibility: {
          ...(prev.sectionVisibility || INITIAL_ADJUSTMENTS.sectionVisibility),
          [sectionName]: true,
        },
      }));
    };

    const handleReset = () => {
      const resetValues: Partial<Adjustments> = {};
      for (const key of sectionKeys) {
        (resetValues as Record<string, unknown>)[key] = JSON.parse(JSON.stringify(INITIAL_ADJUSTMENTS[key]));
      }
      setAdjustments((prev: Adjustments) => ({
        ...prev,
        ...resetValues,
        sectionVisibility: {
          ...(prev.sectionVisibility || INITIAL_ADJUSTMENTS.sectionVisibility),
          [sectionName]: true,
        },
      }));
    };

    const isPasteAllowed = copiedSectionAdjustments && copiedSectionAdjustments.section === sectionName;
    const pasteLabel = copiedSectionAdjustments
      ? t('adjustments.pasteSettings', {
          section: copiedSectionAdjustments.section.charAt(0).toUpperCase() + copiedSectionAdjustments.section.slice(1),
        })
      : t('adjustments.pasteSettings', { section: '' });

    const options: Array<ControlsPanelOption> = [
      {
        label: t('adjustments.copySettings', { section: sectionName.charAt(0).toUpperCase() + sectionName.slice(1) }),
        icon: Copy,
        onClick: handleCopy,
      },
      { label: pasteLabel, icon: ClipboardPaste, onClick: handlePaste, disabled: !isPasteAllowed },
      { type: OPTION_SEPARATOR },
      {
        label: t('adjustments.resetSection', { section: sectionName.charAt(0).toUpperCase() + sectionName.slice(1) }),
        icon: RotateCcw,
        onClick: handleReset,
      },
    ];

    showContextMenu(event.clientX, event.clientY, options);
  };

  return (
    <div className="flex flex-col h-full">
      <div className="p-4 flex justify-between items-center flex-shrink-0 border-b border-surface">
        <h2 className="text-xl font-bold text-primary text-shadow-shiny">{t('adjustments.panelTitle')}</h2>
        <div className="flex items-center gap-1">
          <button
            className="p-2 rounded-full hover:bg-surface disabled:cursor-not-allowed transition-colors"
            disabled={!selectedImage?.isReady}
            onClick={handleAutoAdjustments}
            data-tooltip={t('adjustments.autoAdjust')}
          >
            <Aperture size={18} />
          </button>
          <button
            className="p-2 rounded-full hover:bg-surface disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            disabled={!selectedImage}
            onClick={handleResetAdjustments}
            data-tooltip={t('adjustments.resetAll')}
          >
            <RotateCcw size={18} />
          </button>
        </div>
      </div>
      <div className="flex-grow overflow-y-auto p-4 flex flex-col gap-2">
        {Object.keys(ADJUSTMENT_SECTIONS).map((sectionName: string) => {
          const SectionComponent = (
            {
              basic: BasicAdjustments,
              curves: CurveGraph,
              color: ColorPanel,
              details: DetailsPanel,
              effects: EffectsPanel,
            } as unknown as Record<string, React.ComponentType<Record<string, unknown>>>
          )[sectionName];

          const title = t(`adjustments.${sectionName}`, {
            defaultValue: sectionName.charAt(0).toUpperCase() + sectionName.slice(1),
          });
          const sectionVisibility = adjustments.sectionVisibility || INITIAL_ADJUSTMENTS.sectionVisibility;

          return (
            <div className="flex-shrink-0 group" key={sectionName}>
              <CollapsibleSection
                isContentVisible={sectionVisibility[sectionName]}
                isOpen={collapsibleState[sectionName]}
                onContextMenu={(e: React.MouseEvent) => handleSectionContextMenu(e, sectionName)}
                onToggle={() => handleToggleSection(sectionName)}
                onToggleVisibility={() => handleToggleVisibility(sectionName)}
                title={title}
              >
                <SectionComponent
                  adjustments={adjustments}
                  setAdjustments={setAdjustments}
                  histogram={histogram}
                  theme={theme}
                  handleLutSelect={handleLutSelect}
                  appSettings={appSettings}
                  isWbPickerActive={isWbPickerActive}
                  toggleWbPicker={toggleWbPicker}
                  onDragStateChange={onDragStateChange}
                />
              </CollapsibleSection>
            </div>
          );
        })}
      </div>
    </div>
  );
}
