import type { KeybindDefinition } from '../../utils/keybindContracts';

export const FOCUS_AREAS_INVOKE = 'get_focus_regions';

export const FOCUS_AREA_KEYBIND_DEFINITION: KeybindDefinition = {
  action: 'toggle_focus_areas',
  description: 'Toggle focus area overlay',
  defaultCombo: ['shift', 'KeyF'],
  section: 'view',
};
