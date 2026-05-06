export type FocusKind = 'point' | 'area' | 'face' | 'eye';

export interface FocusRegion {
  x: number;
  y: number;
  width: number;
  height: number;
  kind: FocusKind;
  is_primary: boolean;
}
