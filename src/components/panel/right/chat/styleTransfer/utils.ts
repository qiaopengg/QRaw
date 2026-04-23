import { Adjustments } from '../../../../../utils/adjustments';
import { HslPatch, StyleTransferRequestMode } from '../types';

export function clampStyleTransferConfig(value: number): number {
  return Math.max(0.5, Math.min(2.0, value));
}

export function formatStyleTransferConfig(value: number): string {
  return value.toFixed(2).replace(/\.00$/, '');
}

export function getSimpleAdjustments(adj: Adjustments): Record<string, unknown> {
  return {
    exposure: adj.exposure,
    brightness: adj.brightness,
    contrast: adj.contrast,
    highlights: adj.highlights,
    shadows: adj.shadows,
    whites: adj.whites,
    blacks: adj.blacks,
    saturation: adj.saturation,
    vibrance: adj.vibrance,
    temperature: adj.temperature,
    tint: adj.tint,
    clarity: adj.clarity,
    dehaze: adj.dehaze,
    structure: adj.structure,
    sharpness: adj.sharpness,
    vignetteAmount: adj.vignetteAmount,
    hsl: adj.hsl,
  };
}

export function mergeAdjustments(prev: Adjustments, patch: Partial<Adjustments>): Adjustments {
  const next = { ...prev, ...patch } as Adjustments;
  const patchHsl = patch.hsl as HslPatch | undefined;
  if (!patchHsl || typeof patchHsl !== 'object') return next;
  const prevHsl = prev.hsl;
  const mergedHsl: Adjustments['hsl'] = { ...prevHsl };
  for (const color in patchHsl) {
    const values = patchHsl[color];
    if (!values || typeof values !== 'object') continue;
    mergedHsl[color] = { ...(prevHsl[color] || { hue: 0, saturation: 0, luminance: 0 }), ...values };
  }
  next.hsl = mergedHsl;
  return next;
}

export function getStyleTransferRequestLabel(_mode: StyleTransferRequestMode, t: (key: string) => string): string {
  return t('chat.styleTransferRequest');
}
