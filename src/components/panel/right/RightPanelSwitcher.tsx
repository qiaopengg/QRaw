import { useTranslation } from 'react-i18next';
import { motion } from 'framer-motion';
import { SlidersHorizontal, Info, Scaling, BrushCleaning, Bookmark, Save, Layers } from 'lucide-react';
import { Panel } from '../../ui/AppProperties';

interface PanelOptions {
  icon: any;
  id: Panel;
  title: string;
}

interface RightPanelSwitcherProps {
  activePanel: Panel | null;
  onPanelSelect(id: Panel): void;
  isInstantTransition: boolean;
}

const PANEL_GROUPS: Array<Array<Omit<PanelOptions, 'title'> & { titleKey: string }>> = [
  [{ id: Panel.Metadata, icon: Info, titleKey: 'panels.info' }],
  [
    { id: Panel.Adjustments, icon: SlidersHorizontal, titleKey: 'panels.adjust' },
    { id: Panel.Crop, icon: Scaling, titleKey: 'panels.crop' },
    { id: Panel.Masks, icon: Layers, titleKey: 'panels.masks' },
    { id: Panel.Ai, icon: BrushCleaning, titleKey: 'panels.inpaint' },
  ],
  [
    { id: Panel.Presets, icon: Bookmark, titleKey: 'panels.presets' },
    { id: Panel.Export, icon: Save, titleKey: 'panels.export' },
  ],
];

export default function RightPanelSwitcher({
  activePanel,
  onPanelSelect,
  isInstantTransition,
}: RightPanelSwitcherProps) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col p-1 gap-1 h-full">
      {PANEL_GROUPS.map((group, groupIndex) => (
        <div key={groupIndex} className="flex flex-col gap-1">
          {groupIndex > 0 && <div className="w-6 h-px bg-surface self-center" />}
          {group.map(({ id, icon: Icon, titleKey }) => (
            <button
              className={`relative p-2 rounded-md transition-colors duration-200 ${
                activePanel === id
                  ? 'text-text-primary'
                  : 'text-text-secondary hover:bg-surface hover:text-text-primary'
              }`}
              key={id}
              onClick={() => onPanelSelect(id)}
              data-tooltip={t(titleKey)}
            >
              {activePanel === id && (
                <motion.div
                  layoutId="active-panel-indicator"
                  className="absolute inset-0 bg-surface rounded-md"
                  transition={isInstantTransition ? { duration: 0 } : { type: 'spring', bounce: 0.2, duration: 0.4 }}
                />
              )}
              <Icon size={20} className="relative z-10" />
            </button>
          ))}
        </div>
      ))}
    </div>
  );
}
