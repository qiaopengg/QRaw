export interface KeybindDefinition {
  action: string;
  description: string;
  defaultCombo: string[];
  section: 'library' | 'view' | 'rating' | 'panels' | 'editing';
}

export interface KeybindSection {
  id: KeybindDefinition['section'];
  label: string;
}
