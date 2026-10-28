export interface KeybindDefinition {
  action: string;
  description: string;
  defaultCombo: string[];
  macDefaultCombo?: string[];
  section: 'library' | 'view' | 'rating' | 'panels' | 'editing';
}

export interface KeybindSection {
  id: KeybindDefinition['section'];
  label: string;
}
