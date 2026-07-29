import { Check } from 'lucide-react';
import Text from '../components/ui/Text';
import { TextColors, TextVariants, TextWeights, TEXT_COLOR_KEYS } from '../types/typography';
import type { LibraryFeatureFilterGroup } from './contracts';

interface FeatureFilterOptionsProps {
  groups: LibraryFeatureFilterGroup[];
  selectedFilters: Record<string, string[]>;
  onToggle(groupKey: string, value: string): void;
}

export default function FeatureFilterOptions({ groups, selectedFilters, onToggle }: FeatureFilterOptionsProps) {
  return groups.map((group) => (
    <div key={group.key}>
      <Text as="div" variant={TextVariants.small} weight={TextWeights.semibold} className="px-3 py-1 uppercase">
        {group.label}
      </Text>
      {group.options.map((option) => {
        const isSelected = (selectedFilters[group.key] ?? []).includes(option.value);
        return (
          <button
            className={`w-full text-left px-3 py-2 rounded-md flex items-center justify-between transition-colors duration-150 ${
              isSelected ? 'bg-card-active' : 'hover:bg-bg-primary'
            }`}
            key={option.value}
            onClick={() => onToggle(group.key, option.value)}
            role="menuitem"
          >
            <Text
              variant={TextVariants.label}
              color={TextColors.primary}
              weight={isSelected ? TextWeights.semibold : TextWeights.normal}
            >
              {option.label}
            </Text>
            {isSelected && <Check size={16} className={TEXT_COLOR_KEYS[TextColors.primary]} />}
          </button>
        );
      })}
    </div>
  ));
}
