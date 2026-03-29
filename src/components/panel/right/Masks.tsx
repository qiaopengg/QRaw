import React from 'react';
import {
  Brush,
  Circle,
  Cloud,
  Droplet,
  Eraser,
  Layers,
  RectangleHorizontal,
  Sparkles,
  TriangleRight,
  User,
} from 'lucide-react';

export enum Mask {
  AiForeground = 'ai-foreground',
  AiSky = 'ai-sky',
  AiSubject = 'ai-subject',
  All = 'all',
  Brush = 'brush',
  Color = 'color',
  Linear = 'linear',
  Luminance = 'luminance',
  QuickEraser = 'quick-eraser',
  Radial = 'radial',
}

export enum SubMaskMode {
  Additive = 'additive',
  Subtractive = 'subtractive',
}

export enum ToolType {
  AiSeletor = 'ai-selector',
  Brush = 'brush',
  Eraser = 'eraser',
  GenerativeReplace = 'generative-replace',
  SelectSubject = 'select-subject',
}

export interface MaskType {
  disabled: boolean;
  icon: React.ComponentType<{ size?: number; className?: string }>;
  id?: string;
  name: string;
  type: Mask | null;
}

export interface SubMask {
  id: string;
  invert: boolean;
  mode: SubMaskMode;
  name?: string;
  opacity: number;
  parameters?: Record<string, unknown>;
  type: Mask;
  visible: boolean;
}

export function formatMaskTypeName(type: string) {
  if (type === Mask.AiSubject) return 'AI Subject';
  if (type === Mask.AiForeground) return 'AI Foreground';
  if (type === Mask.AiSky) return 'AI Sky';
  if (type === Mask.All) return 'Whole Image';
  return type.charAt(0).toUpperCase() + type.slice(1);
}

export function getSubMaskName(subMask: Pick<SubMask, 'name' | 'type'>) {
  return subMask.name?.trim() || formatMaskTypeName(subMask.type);
}

export const MASK_ICON_MAP: Record<Mask, React.ComponentType<{ size?: number; className?: string }>> = {
  [Mask.AiForeground]: User,
  [Mask.AiSky]: Cloud,
  [Mask.AiSubject]: Sparkles,
  [Mask.All]: RectangleHorizontal,
  [Mask.Brush]: Brush,
  [Mask.Color]: Droplet,
  [Mask.Linear]: TriangleRight,
  [Mask.Luminance]: Sparkles,
  [Mask.QuickEraser]: Eraser,
  [Mask.Radial]: Circle,
};

export const getMaskPanelCreationTypes = (t: (key: string) => string): Array<MaskType> => [
  {
    disabled: false,
    icon: Sparkles,
    name: t('masking.subject'),
    type: Mask.AiSubject,
  },
  {
    disabled: false,
    icon: Cloud,
    name: t('masking.sky'),
    type: Mask.AiSky,
  },
  {
    disabled: false,
    icon: User,
    name: t('masking.foreground'),
    type: Mask.AiForeground,
  },
  {
    disabled: false,
    icon: TriangleRight,
    name: t('masking.linear'),
    type: Mask.Linear,
  },
  {
    disabled: false,
    icon: Circle,
    name: t('masking.radial'),
    type: Mask.Radial,
  },
  {
    disabled: false,
    icon: Layers,
    id: 'others',
    name: t('masking.others'),
    type: null,
  },
];

export const getAiPanelCreationTypes = (t: (key: string) => string): Array<MaskType> => [
  {
    disabled: false,
    icon: Eraser,
    name: t('masking.quickErase'),
    type: Mask.QuickEraser,
  },
  {
    disabled: false,
    icon: Sparkles,
    name: t('masking.subject'),
    type: Mask.AiSubject,
  },
  {
    disabled: false,
    icon: User,
    name: t('masking.foreground'),
    type: Mask.AiForeground,
  },
  {
    disabled: false,
    icon: Brush,
    name: t('masking.brushType'),
    type: Mask.Brush,
  },
  {
    disabled: false,
    icon: TriangleRight,
    name: t('masking.linear'),
    type: Mask.Linear,
  },
  {
    disabled: false,
    icon: Circle,
    name: t('masking.radial'),
    type: Mask.Radial,
  },
];

export const getSubMaskComponentTypes = (t: (key: string) => string): Array<MaskType> => [
  {
    disabled: false,
    icon: Sparkles,
    name: t('masking.subject'),
    type: Mask.AiSubject,
  },
  {
    disabled: false,
    icon: Cloud,
    name: t('masking.sky'),
    type: Mask.AiSky,
  },
  {
    disabled: false,
    icon: User,
    name: t('masking.foreground'),
    type: Mask.AiForeground,
  },
  {
    disabled: false,
    icon: TriangleRight,
    name: t('masking.linear'),
    type: Mask.Linear,
  },
  {
    disabled: false,
    icon: Circle,
    name: t('masking.radial'),
    type: Mask.Radial,
  },
  {
    disabled: false,
    icon: Layers,
    id: 'others',
    name: t('masking.others'),
    type: null,
  },
];

export const getOthersMaskTypes = (t: (key: string) => string): Array<MaskType> => [
  {
    disabled: false,
    icon: Brush,
    name: t('masking.brushType'),
    type: Mask.Brush,
  },
  {
    disabled: false,
    icon: RectangleHorizontal,
    name: t('masking.wholeImage'),
    type: Mask.All,
  },
];

export const getAiSubMaskComponentTypes = (t: (key: string) => string): Array<MaskType> => [
  {
    disabled: false,
    icon: Sparkles,
    name: t('masking.subject'),
    type: Mask.AiSubject,
  },
  {
    disabled: false,
    icon: User,
    name: t('masking.foreground'),
    type: Mask.AiForeground,
  },
  {
    disabled: false,
    icon: Brush,
    name: t('masking.brushType'),
    type: Mask.Brush,
  },
  {
    disabled: false,
    icon: TriangleRight,
    name: t('masking.linear'),
    type: Mask.Linear,
  },
  {
    disabled: false,
    icon: Circle,
    name: t('masking.radial'),
    type: Mask.Radial,
  },
];
