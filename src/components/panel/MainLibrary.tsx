import { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle,
  Check,
  Folder,
  FolderInput,
  Home,
  Image as ImageIcon,
  Loader2,
  FolderOpen,
  RefreshCw,
  Settings,
  SlidersHorizontal,
  Star as StarIcon,
  Search,
  Users,
  X,
} from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { List, useListCallbackRef } from 'react-window';
import AutoSizer from 'react-virtualized-auto-sizer';
import Button from '../ui/Button';
import SettingsPanel from './SettingsPanel';
import { ThemeProps, THEMES, DEFAULT_THEME_ID } from '../../utils/themes';
import {
  AppSettings,
  FilterCriteria,
  ImageFile,
  Invokes,
  LibraryViewMode,
  Progress,
  RawStatus,
  SortCriteria,
  SortDirection,
  SupportedTypes,
  ThumbnailSize,
  ThumbnailAspectRatio,
} from '../ui/AppProperties';
import { Color, COLOR_LABELS } from '../../utils/adjustments';
import { ImportState, Status } from '../ui/ExportImportProperties';
import Text from '../ui/Text';
import { TextColors, TextVariants, TextWeights } from '../../types/typography';

interface DropdownMenuProps {
  buttonContent: React.ReactNode;
  buttonTitle: string;
  children: React.ReactNode;
  contentClassName: string;
}

interface FilterOptionProps {
  filterCriteria: FilterCriteria;
  setFilterCriteria(criteria: FilterCriteria | ((prev: FilterCriteria) => FilterCriteria)): void;
}

interface KeyValueLabel {
  key?: string;
  label?: string;
  value?: number;
}

interface SearchCriteria {
  tags: string[];
  text: string;
  mode: 'AND' | 'OR';
}

interface MainLibraryProps {
  activePath: string | null;
  aiModelDownloadStatus: string | null;
  appSettings: AppSettings | null;
  currentFolderPath: string | null;
  filterCriteria: FilterCriteria;
  imageList: Array<ImageFile>;
  imageRatings: Record<string, number>;
  importState: ImportState;
  indexingProgress: Progress;
  isLoading: boolean;
  isThumbnailsLoading?: boolean;
  isIndexing: boolean;
  isTreeLoading: boolean;
  libraryScrollTop: number;
  libraryViewMode: LibraryViewMode;
  multiSelectedPaths: Array<string>;
  onClearSelection(): void;
  onContextMenu(event: React.MouseEvent, path: string): void;
  onContinueSession(): void;
  onEmptyAreaContextMenu(event: React.MouseEvent): void;
  onGoHome(): void;
  onImageClick(path: string, event: React.MouseEvent): void;
  onImageDoubleClick(path: string): void;
  onLibraryRefresh(): void;
  onOpenFolder(): void;
  onSettingsChange(settings: AppSettings): void;
  onThumbnailAspectRatioChange(aspectRatio: ThumbnailAspectRatio): void;
  onThumbnailSizeChange(size: ThumbnailSize): void;
  rootPath: string | null;
  searchCriteria: SearchCriteria;
  setFilterCriteria(criteria: FilterCriteria): void;
  setLibraryScrollTop(scrollTop: number): void;
  setLibraryViewMode(mode: LibraryViewMode): void;
  setSearchCriteria(criteria: SearchCriteria | ((prev: SearchCriteria) => SearchCriteria)): void;
  setSortCriteria(criteria: SortCriteria | ((prev: SortCriteria) => SortCriteria)): void;
  sortCriteria: SortCriteria;
  theme: string;
  thumbnailAspectRatio: ThumbnailAspectRatio;
  thumbnails: Record<string, string>;
  thumbnailSize: ThumbnailSize;
  onNavigateToCommunity(): void;
}

interface SearchInputProps {
  indexingProgress: Progress;
  isIndexing: boolean;
  searchCriteria: SearchCriteria;
  setSearchCriteria(criteria: SearchCriteria | ((prev: SearchCriteria) => SearchCriteria)): void;
}

interface SortOptionsProps {
  sortCriteria: SortCriteria;
  setSortCriteria(criteria: SortCriteria | ((prev: SortCriteria) => SortCriteria)): void;
  sortOptions: Array<Omit<SortCriteria, 'order'> & { label?: string; disabled?: boolean }>;
}

interface ImageLayer {
  id: string;
  url: string;
  opacity: number;
}

interface ThumbnailProps {
  data: string | undefined;
  isActive: boolean;
  isSelected: boolean;
  onContextMenu(e: React.MouseEvent): void;
  onImageClick(path: string, event: React.MouseEvent): void;
  onImageDoubleClick(path: string): void;
  onLoad(): void;
  path: string;
  rating: number;
  tags: Array<string> | null;
  aspectRatio: ThumbnailAspectRatio;
}

interface ThumbnailSizeOption {
  id: ThumbnailSize;
  label: string;
  size: number;
}

interface ThumbnailSizeProps {
  onSelectSize(sizeOptions: ThumbnailSize): void;
  selectedSize: ThumbnailSize;
}

interface ThumbnailAspectRatioOption {
  id: ThumbnailAspectRatio;
  label: string;
}

interface ThumbnailAspectRatioProps {
  onSelectAspectRatio(aspectRatio: ThumbnailAspectRatio): void;
  selectedAspectRatio: ThumbnailAspectRatio;
}

interface ViewOptionsProps {
  filterCriteria: FilterCriteria;
  libraryViewMode: LibraryViewMode;
  onSelectSize(size: ThumbnailSize): void;
  onSelectAspectRatio(aspectRatio: ThumbnailAspectRatio): void;
  setFilterCriteria(criteria: FilterCriteria | ((prev: FilterCriteria) => FilterCriteria)): void;
  setLibraryViewMode(mode: LibraryViewMode): void;
  setSortCriteria(criteria: SortCriteria): void;
  sortCriteria: SortCriteria;
  sortOptions: Array<Omit<SortCriteria, 'order'> & { label?: string; disabled?: boolean }>;
  thumbnailSize: ThumbnailSize;
  thumbnailAspectRatio: ThumbnailAspectRatio;
}

const THUMBNAIL_SIZE_VALUES: Array<{ id: ThumbnailSize; size: number }> = [
  { id: ThumbnailSize.Small, size: 160 },
  { id: ThumbnailSize.Medium, size: 240 },
  { id: ThumbnailSize.Large, size: 320 },
];

const getRatingFilterOptions = (
  t: (key: string, options?: Record<string, unknown>) => string,
): Array<KeyValueLabel> => [
  { value: 0, label: t('library.showAll') },
  { value: 1, label: t('library.ratingAndUp', { count: 1 }) },
  { value: 2, label: t('library.ratingAndUp', { count: 2 }) },
  { value: 3, label: t('library.ratingAndUp', { count: 3 }) },
  { value: 4, label: t('library.ratingAndUp', { count: 4 }) },
  { value: 5, label: t('library.ratingOnly', { count: 5 }) },
];

const getRawStatusOptions = (t: (key: string) => string): Array<KeyValueLabel> => [
  { key: RawStatus.All, label: t('library.allTypes') },
  { key: RawStatus.RawOnly, label: t('library.rawOnly') },
  { key: RawStatus.NonRawOnly, label: t('library.nonRawOnly') },
  { key: RawStatus.RawOverNonRaw, label: t('library.preferRaw') },
];

const getThumbnailSizeOptions = (t: (key: string) => string): Array<ThumbnailSizeOption> => [
  { id: ThumbnailSize.Small, label: t('library.small'), size: 160 },
  { id: ThumbnailSize.Medium, label: t('library.medium'), size: 240 },
  { id: ThumbnailSize.Large, label: t('library.large'), size: 320 },
];

const getThumbnailAspectRatioOptions = (t: (key: string) => string): Array<ThumbnailAspectRatioOption> => [
  { id: ThumbnailAspectRatio.Cover, label: t('library.fillSquare') },
  { id: ThumbnailAspectRatio.Contain, label: t('library.originalRatio') },
];

const groupImagesByFolder = (images: ImageFile[], rootPath: string | null) => {
  const groups: Record<string, ImageFile[]> = {};

  images.forEach((img) => {
    const physicalPath = img.path.split('?vc=')[0];
    const separator = physicalPath.includes('/') ? '/' : '\\';
    const lastSep = physicalPath.lastIndexOf(separator);
    const dir = lastSep > -1 ? physicalPath.substring(0, lastSep) : physicalPath;

    if (!groups[dir]) {
      groups[dir] = [];
    }
    groups[dir].push(img);
  });

  const sortedKeys = Object.keys(groups).sort((a, b) => {
    if (a === rootPath) return -1;
    if (b === rootPath) return 1;
    return a.localeCompare(b);
  });

  return sortedKeys.map((dir) => ({
    path: dir,
    images: groups[dir],
  }));
};

function SearchInput({ indexingProgress, isIndexing, searchCriteria, setSearchCriteria }: SearchInputProps) {
  const { t } = useTranslation();
  const [isSearchActive, setIsSearchActive] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const { tags, text, mode } = searchCriteria;

  const [contentWidth, setContentWidth] = useState(0);

  useEffect(() => {
    if (isSearchActive) {
      inputRef.current?.focus();
    }
  }, [isSearchActive]);

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(event.target as Node) && tags.length === 0 && !text) {
        setIsSearchActive(false);
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [tags, text]);

  useEffect(() => {
    if (contentRef.current) {
      const timer = setTimeout(() => {
        if (contentRef.current) {
          setContentWidth(contentRef.current.scrollWidth);
        }
      }, 0);
      return () => clearTimeout(timer);
    }
  }, [tags, text, isSearchActive]);

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setSearchCriteria((prev) => ({ ...prev, text: e.target.value }));
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if ((e.key === ',' || e.key === 'Enter') && text.trim()) {
      e.preventDefault();
      setSearchCriteria((prev) => ({
        ...prev,
        tags: [...prev.tags, text.trim()],
        text: '',
      }));
    } else if (e.key === 'Backspace' && !text && tags.length > 0) {
      e.preventDefault();
      const lastTag = tags[tags.length - 1];
      setSearchCriteria((prev) => ({
        ...prev,
        tags: prev.tags.slice(0, -1),
        text: lastTag,
      }));
    }
  };

  const removeTag = (tagToRemove: string) => {
    setSearchCriteria((prev) => ({
      ...prev,
      tags: prev.tags.filter((tag) => tag !== tagToRemove),
    }));
  };

  const clearSearch = () => {
    setSearchCriteria({ tags: [], text: '', mode: 'OR' });
    setIsSearchActive(false);
    inputRef.current?.blur();
  };

  const toggleMode = () => {
    setSearchCriteria((prev) => ({
      ...prev,
      mode: prev.mode === 'AND' ? 'OR' : 'AND',
    }));
  };

  const isActive = isSearchActive || tags.length > 0 || !!text;
  const placeholderText =
    isIndexing && indexingProgress.total > 0
      ? t('library.indexingProgress', { current: indexingProgress.current, total: indexingProgress.total })
      : isIndexing
        ? t('library.indexingImages')
        : tags.length > 0
          ? t('library.addAnotherTag')
          : t('library.searchPlaceholder');

  const INACTIVE_WIDTH = 48;
  const PADDING_AND_ICONS_WIDTH = 105;
  const MAX_WIDTH = 640;

  const calculatedWidth = Math.min(MAX_WIDTH, contentWidth + PADDING_AND_ICONS_WIDTH);

  return (
    <motion.div
      animate={{ width: isActive ? calculatedWidth : INACTIVE_WIDTH }}
      className="relative flex items-center bg-surface rounded-md h-12"
      initial={false}
      layout
      ref={containerRef}
      transition={{ type: 'spring', stiffness: 400, damping: 35 }}
      onClick={() => inputRef.current?.focus()}
    >
      <button
        className="absolute left-0 top-0 h-12 w-12 flex items-center justify-center text-text-primary z-10 flex-shrink-0"
        onClick={(e) => {
          e.stopPropagation();
          if (!isActive) {
            setIsSearchActive(true);
          }
          inputRef.current?.focus();
        }}
        data-tooltip={t('library.search')}
      >
        <Search className="w-4 h-4" />
      </button>

      <div
        className="flex items-center gap-1 pl-12 pr-16 w-full h-full overflow-x-hidden"
        style={{ opacity: isActive ? 1 : 0, pointerEvents: isActive ? 'auto' : 'none', transition: 'opacity 0.2s' }}
      >
        <div ref={contentRef} className="flex items-center gap-2 h-full flex-nowrap min-w-[300px]">
          {tags.map((tag) => (
            <motion.div
              key={tag}
              layout
              initial={{ opacity: 0, scale: 0.5 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.5 }}
              className="flex items-center gap-1 bg-bg-primary px-2 py-1 rounded group cursor-pointer flex-shrink-0"
              onClick={(e) => {
                e.stopPropagation();
                removeTag(tag);
              }}
            >
              <Text variant={TextVariants.small} color={TextColors.primary} weight={TextWeights.medium}>
                {tag}
              </Text>
              <span className="rounded-full group-hover:bg-black/20 p-0.5 transition-colors">
                <X size={12} />
              </span>
            </motion.div>
          ))}
          <input
            className="flex-grow w-full h-full bg-transparent text-text-primary placeholder-text-secondary border-none focus:outline-none"
            disabled={isIndexing}
            onBlur={() => {
              if (tags.length === 0 && !text) {
                setIsSearchActive(false);
              }
            }}
            onChange={handleInputChange}
            onFocus={() => setIsSearchActive(true)}
            onKeyDown={handleKeyDown}
            placeholder={placeholderText}
            ref={inputRef}
            type="text"
            value={text}
          />
        </div>
      </div>

      <div
        className="absolute inset-y-0 right-0 flex items-center gap-1 pr-2"
        style={{ opacity: isActive ? 1 : 0, pointerEvents: isActive ? 'auto' : 'none', transition: 'opacity 0.2s' }}
      >
        <AnimatePresence>
          {text.trim().length > 0 && tags.length === 0 && text.trim().length < 6 && !isIndexing && (
            <motion.div
              initial={{ opacity: 0, scale: 0.8 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.8 }}
              transition={{ duration: 0.15 }}
              className="flex-shrink-0 bg-bg-primary px-2 py-1 rounded-md whitespace-nowrap"
            >
              <Text variant={TextVariants.small}>
                {t('library.separateTagsWith')} <kbd className="font-sans font-semibold">,</kbd>
              </Text>
            </motion.div>
          )}
        </AnimatePresence>

        {tags.length > 0 && (
          <button
            onClick={toggleMode}
            className="p-1.5 rounded-md hover:bg-bg-primary w-10 flex-shrink-0"
            data-tooltip={`${t('library.match')} ${mode === 'AND' ? t('library.matchAll') : t('library.matchAny')} ${t('library.tags')}`}
          >
            <Text variant={TextVariants.small} color={TextColors.primary} weight={TextWeights.semibold}>
              {mode}
            </Text>
          </button>
        )}
        {(tags.length > 0 || text) && !isIndexing && (
          <button
            onClick={clearSearch}
            className="p-1.5 rounded-md text-text-secondary hover:text-text-primary hover:bg-bg-primary flex-shrink-0"
            data-tooltip={t('folderTree.clearSearch')}
          >
            <X className="h-5 w-5" />
          </button>
        )}
        {isIndexing && (
          <div className="flex items-center pr-1 pointer-events-none flex-shrink-0">
            <Loader2 className="h-5 w-5 text-text-secondary animate-spin" />
          </div>
        )}
      </div>
    </motion.div>
  );
}

function ColorFilterOptions({ filterCriteria, setFilterCriteria }: FilterOptionProps) {
  const { t } = useTranslation();
  const [lastClickedColor, setLastClickedColor] = useState<string | null>(null);
  const allColors = useMemo(() => [...COLOR_LABELS, { name: 'none', color: '#9ca3af' }], []);

  const handleColorClick = (colorName: string, event: React.MouseEvent) => {
    const { ctrlKey, metaKey, shiftKey } = event;
    const isCtrlPressed = ctrlKey || metaKey;
    const currentColors = filterCriteria.colors || [];

    if (shiftKey && lastClickedColor) {
      const lastIndex = allColors.findIndex((c) => c.name === lastClickedColor);
      const currentIndex = allColors.findIndex((c) => c.name === colorName);
      if (lastIndex !== -1 && currentIndex !== -1) {
        const start = Math.min(lastIndex, currentIndex);
        const end = Math.max(lastIndex, currentIndex);
        const range = allColors.slice(start, end + 1).map((c: Color) => c.name);
        const baseSelection = isCtrlPressed ? currentColors : [lastClickedColor];
        const newColors = Array.from(new Set([...baseSelection, ...range]));
        setFilterCriteria((prev: FilterCriteria) => ({ ...prev, colors: newColors }));
      }
    } else if (isCtrlPressed) {
      const newColors = currentColors.includes(colorName)
        ? currentColors.filter((c: string) => c !== colorName)
        : [...currentColors, colorName];
      setFilterCriteria((prev: FilterCriteria) => ({ ...prev, colors: newColors }));
    } else {
      const newColors = currentColors.length === 1 && currentColors[0] === colorName ? [] : [colorName];
      setFilterCriteria((prev: FilterCriteria) => ({ ...prev, colors: newColors }));
    }
    setLastClickedColor(colorName);
  };

  return (
    <div>
      <Text as="div" variant={TextVariants.small} weight={TextWeights.semibold} className="px-3 py-2 uppercase">
        {t('library.filterByColorLabel')}
      </Text>
      <div className="flex flex-wrap gap-3 px-3 py-2">
        {allColors.map((color: Color) => {
          const isSelected = (filterCriteria.colors || []).includes(color.name);
          const title =
            color.name === 'none' ? t('library.noLabel') : color.name.charAt(0).toUpperCase() + color.name.slice(1);
          return (
            <button
              key={color.name}
              data-tooltip={title}
              onClick={(e: React.MouseEvent) => handleColorClick(color.name, e)}
              className="w-6 h-6 rounded-full focus:outline-none focus:ring-2 focus:ring-accent focus:ring-offset-2 focus:ring-offset-surface transition-transform hover:scale-110"
              role="menuitem"
            >
              <div className="relative w-full h-full">
                <div className="w-full h-full rounded-full" style={{ backgroundColor: color.color }}></div>
                {isSelected && (
                  <div className="absolute inset-0 flex items-center justify-center bg-black/30 rounded-full">
                    <Check size={14} className="text-white" />
                  </div>
                )}
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}

function DropdownMenu({ buttonContent, buttonTitle, children, contentClassName = 'w-56' }: DropdownMenuProps) {
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  return (
    <div className="relative" ref={dropdownRef}>
      <Button
        aria-expanded={isOpen}
        aria-haspopup="true"
        className="h-12 w-12 bg-surface text-text-primary shadow-none p-0 flex items-center justify-center"
        onClick={() => setIsOpen(!isOpen)}
        data-tooltip={buttonTitle}
      >
        {buttonContent}
      </Button>
      <AnimatePresence>
        {isOpen && (
          <motion.div
            className={`absolute right-0 mt-2 ${contentClassName} origin-top-right z-20`}
            initial={{ opacity: 0, scale: 0.95 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.95 }}
            transition={{ duration: 0.1, ease: 'easeOut' }}
          >
            <div
              className="bg-surface/90 backdrop-blur-md rounded-lg shadow-xl"
              role="menu"
              aria-orientation="vertical"
            >
              {children}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function ThumbnailSizeOptions({ selectedSize, onSelectSize }: ThumbnailSizeProps) {
  const { t } = useTranslation();
  const thumbnailSizeOptions = getThumbnailSizeOptions(t);
  return (
    <>
      <Text as="div" variant={TextVariants.small} weight={TextWeights.semibold} className="px-3 py-2 uppercase">
        {t('library.thumbnailSize')}
      </Text>
      {thumbnailSizeOptions.map((option: ThumbnailSizeOption) => {
        const isSelected = selectedSize === option.id;
        return (
          <button
            className={`w-full text-left px-3 py-2 rounded-md flex items-center justify-between transition-colors duration-150 ${
              isSelected ? 'bg-card-active' : 'hover:bg-bg-primary'
            }`}
            key={option.id}
            onClick={() => onSelectSize(option.id)}
            role="menuitem"
          >
            <Text
              variant={TextVariants.label}
              color={TextColors.primary}
              weight={isSelected ? TextWeights.semibold : TextWeights.normal}
            >
              {option.label}
            </Text>
            {isSelected && <Check size={16} />}
          </button>
        );
      })}
    </>
  );
}

function ThumbnailAspectRatioOptions({ selectedAspectRatio, onSelectAspectRatio }: ThumbnailAspectRatioProps) {
  const { t } = useTranslation();
  const thumbnailAspectRatioOptions = getThumbnailAspectRatioOptions(t);
  return (
    <>
      <Text as="div" variant={TextVariants.small} weight={TextWeights.semibold} className="px-3 py-2 uppercase">
        {t('library.thumbnailFit')}
      </Text>
      {thumbnailAspectRatioOptions.map((option: ThumbnailAspectRatioOption) => {
        const isSelected = selectedAspectRatio === option.id;
        return (
          <button
            className={`w-full text-left px-3 py-2 rounded-md flex items-center justify-between transition-colors duration-150 ${
              isSelected ? 'bg-card-active' : 'hover:bg-bg-primary'
            }`}
            key={option.id}
            onClick={() => onSelectAspectRatio(option.id)}
            role="menuitem"
          >
            <Text
              variant={TextVariants.label}
              color={TextColors.primary}
              weight={isSelected ? TextWeights.semibold : TextWeights.normal}
            >
              {option.label}
            </Text>
            {isSelected && <Check size={16} />}
          </button>
        );
      })}
    </>
  );
}

function FilterOptions({ filterCriteria, setFilterCriteria }: FilterOptionProps) {
  const { t } = useTranslation();
  const ratingFilterOptions = getRatingFilterOptions(t);
  const rawStatusOptions = getRawStatusOptions(t);

  const handleRatingFilterChange = (rating: number | undefined) => {
    setFilterCriteria((prev: FilterCriteria) => ({ ...prev, rating: rating ?? 0 }));
  };

  const handleRawStatusChange = (rawStatus: RawStatus | undefined) => {
    setFilterCriteria((prev: FilterCriteria) => ({ ...prev, rawStatus: rawStatus ?? RawStatus.All }));
  };

  return (
    <>
      <div className="space-y-4">
        <div>
          <Text as="div" variant={TextVariants.small} weight={TextWeights.semibold} className="px-3 py-2 uppercase">
            {t('library.filterByRating')}
          </Text>
          {ratingFilterOptions.map((option: KeyValueLabel) => {
            const isSelected = filterCriteria.rating === option.value;
            return (
              <button
                className={`w-full text-left px-3 py-2 rounded-md flex items-center justify-between transition-colors duration-150 ${
                  isSelected ? 'bg-card-active' : 'hover:bg-bg-primary'
                }`}
                key={option.value}
                onClick={() => handleRatingFilterChange(option.value)}
                role="menuitem"
              >
                <span className="flex items-center gap-2">
                  {option.value && option.value > 0 && <StarIcon size={16} className="text-accent fill-accent" />}
                  <Text
                    variant={TextVariants.label}
                    color={TextColors.primary}
                    weight={isSelected ? TextWeights.semibold : TextWeights.normal}
                  >
                    {option.label}
                  </Text>
                </span>
                {isSelected && <Check size={16} />}
              </button>
            );
          })}
        </div>

        <div>
          <Text as="div" variant={TextVariants.small} weight={TextWeights.semibold} className="px-3 py-2 uppercase">
            {t('library.filterByFileType')}
          </Text>
          {rawStatusOptions.map((option: KeyValueLabel) => {
            const isSelected = (filterCriteria.rawStatus || RawStatus.All) === option.key;
            return (
              <button
                className={`w-full text-left px-3 py-2 rounded-md flex items-center justify-between transition-colors duration-150 ${
                  isSelected ? 'bg-card-active' : 'hover:bg-bg-primary'
                }`}
                key={option.key}
                onClick={() => handleRawStatusChange(option.key as RawStatus)}
                role="menuitem"
              >
                <Text
                  variant={TextVariants.label}
                  color={TextColors.primary}
                  weight={isSelected ? TextWeights.semibold : TextWeights.normal}
                >
                  {option.label}
                </Text>
                {isSelected && <Check size={16} />}
              </button>
            );
          })}
        </div>
      </div>
      <div className="py-2"></div>
      <ColorFilterOptions filterCriteria={filterCriteria} setFilterCriteria={setFilterCriteria} />
    </>
  );
}

function SortOptions({ sortCriteria, setSortCriteria, sortOptions }: SortOptionsProps) {
  const { t } = useTranslation();

  const handleKeyChange = (key: string) => {
    setSortCriteria((prev: SortCriteria) => ({ ...prev, key }));
  };

  const handleOrderToggle = () => {
    setSortCriteria((prev: SortCriteria) => ({
      ...prev,
      order: prev.order === SortDirection.Ascending ? SortDirection.Descening : SortDirection.Ascending,
    }));
  };

  return (
    <>
      <div className="px-3 py-2 relative flex items-center">
        <Text as="div" variant={TextVariants.small} weight={TextWeights.semibold} className="uppercase">
          {t('library.sortBy')}
        </Text>
        <button
          onClick={handleOrderToggle}
          data-tooltip={`${t('library.sort')} ${sortCriteria.order === SortDirection.Ascending ? t('library.descending') : t('library.ascending')}`}
          className="absolute top-1/2 right-3 -translate-y-1/2 p-1 bg-transparent border-none text-text-secondary hover:text-text-primary focus:outline-none focus:ring-1 focus:ring-accent rounded"
        >
          {sortCriteria.order === SortDirection.Ascending ? (
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="m18 15-6-6-6 6" />
            </svg>
          ) : (
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="m6 9 6 6 6-6" />
            </svg>
          )}
        </button>
      </div>
      {sortOptions.map((option) => {
        const isSelected = sortCriteria.key === option.key;
        return (
          <button
            className={`w-full text-left px-3 py-2 rounded-md flex items-center justify-between transition-colors duration-150 ${
              isSelected ? 'bg-card-active' : 'hover:bg-bg-primary'
            } ${option.disabled ? 'opacity-50 cursor-not-allowed' : ''}`}
            key={option.key}
            onClick={() => !option.disabled && handleKeyChange(option.key)}
            role="menuitem"
            disabled={option.disabled}
            data-tooltip={option.disabled ? t('library.enableExifTooltip') : undefined}
          >
            <Text
              variant={TextVariants.label}
              color={TextColors.primary}
              weight={isSelected ? TextWeights.semibold : TextWeights.normal}
            >
              {option.label}
            </Text>
            {isSelected && <Check size={16} />}
          </button>
        );
      })}
    </>
  );
}

function ViewModeOptions({ mode, setMode }: { mode: LibraryViewMode; setMode: (m: LibraryViewMode) => void }) {
  const { t } = useTranslation();
  return (
    <>
      <Text as="div" variant={TextVariants.small} weight={TextWeights.semibold} className="px-3 py-2 uppercase">
        {t('library.displayMode')}
      </Text>
      <button
        className={`w-full text-left px-3 py-2 rounded-md flex items-center justify-between transition-colors duration-150 ${
          mode === LibraryViewMode.Flat ? 'bg-card-active' : 'hover:bg-bg-primary'
        }`}
        onClick={() => setMode(LibraryViewMode.Flat)}
        role="menuitem"
      >
        <Text
          variant={TextVariants.label}
          color={TextColors.primary}
          weight={mode === LibraryViewMode.Flat ? TextWeights.semibold : TextWeights.normal}
        >
          {t('library.currentFolder')}
        </Text>
        {mode === LibraryViewMode.Flat && <Check size={16} />}
      </button>
      <button
        className={`w-full text-left px-3 py-2 rounded-md flex items-center justify-between transition-colors duration-150 ${
          mode === LibraryViewMode.Recursive ? 'bg-card-active' : 'hover:bg-bg-primary'
        }`}
        onClick={() => setMode(LibraryViewMode.Recursive)}
        role="menuitem"
      >
        <Text
          variant={TextVariants.label}
          color={TextColors.primary}
          weight={mode === LibraryViewMode.Recursive ? TextWeights.semibold : TextWeights.normal}
        >
          {t('library.recursive')}
        </Text>
        {mode === LibraryViewMode.Recursive && <Check size={16} />}
      </button>
    </>
  );
}

function ViewOptionsDropdown({
  filterCriteria,
  libraryViewMode,
  onSelectSize,
  onSelectAspectRatio,
  setFilterCriteria,
  setLibraryViewMode,
  setSortCriteria,
  sortCriteria,
  sortOptions,
  thumbnailSize,
  thumbnailAspectRatio,
}: ViewOptionsProps) {
  const { t } = useTranslation();
  const isFilterActive =
    filterCriteria.rating > 0 ||
    (filterCriteria.rawStatus && filterCriteria.rawStatus !== RawStatus.All) ||
    (filterCriteria.colors && filterCriteria.colors.length > 0);

  return (
    <DropdownMenu
      buttonContent={
        <>
          <SlidersHorizontal className="w-8 h-8" />
          {isFilterActive && <div className="absolute -top-1 -right-1 bg-accent rounded-full w-3 h-3" />}
        </>
      }
      buttonTitle={t('library.viewOptions')}
      contentClassName="w-[720px]"
    >
      <div className="flex">
        <div className="w-1/4 p-2 border-r border-border-color">
          <ThumbnailSizeOptions selectedSize={thumbnailSize} onSelectSize={onSelectSize} />
          <div className="pt-2">
            <ThumbnailAspectRatioOptions
              selectedAspectRatio={thumbnailAspectRatio}
              onSelectAspectRatio={onSelectAspectRatio}
            />
          </div>
          <div className="pt-2">
            <ViewModeOptions mode={libraryViewMode} setMode={setLibraryViewMode} />
          </div>
        </div>
        <div className="w-2/4 p-2 border-r border-border-color">
          <FilterOptions filterCriteria={filterCriteria} setFilterCriteria={setFilterCriteria} />
        </div>
        <div className="w-1/4 p-2">
          <SortOptions sortCriteria={sortCriteria} setSortCriteria={setSortCriteria} sortOptions={sortOptions} />
        </div>
      </div>
    </DropdownMenu>
  );
}

function Thumbnail({
  data,
  isActive,
  isSelected,
  onContextMenu,
  onImageClick,
  onImageDoubleClick,
  onLoad,
  path,
  rating,
  tags,
  aspectRatio: thumbnailAspectRatio,
}: ThumbnailProps) {
  const { t } = useTranslation();
  const [showPlaceholder, setShowPlaceholder] = useState(false);
  const [layers, setLayers] = useState<ImageLayer[]>([]);
  const latestThumbDataRef = useRef<string | undefined>(undefined);

  const { baseName, isVirtualCopy } = useMemo(() => {
    const fullFileName = path.split(/[\\/]/).pop() || '';
    const parts = fullFileName.split('?vc=');
    return {
      baseName: parts[0],
      isVirtualCopy: parts.length > 1,
    };
  }, [path]);

  useEffect(() => {
    if (data) {
      setShowPlaceholder(false);
      return;
    }
    const timer = setTimeout(() => {
      setShowPlaceholder(true);
    }, 500);
    return () => clearTimeout(timer);
  }, [data]);

  useEffect(() => {
    if (!data) {
      setLayers([]);
      latestThumbDataRef.current = undefined;
      return;
    }

    if (data !== latestThumbDataRef.current) {
      latestThumbDataRef.current = data;

      setLayers((prev) => {
        if (prev.some((l) => l.id === data)) {
          return prev;
        }
        return [...prev, { id: data, url: data, opacity: 0 }];
      });
    }
  }, [data]);

  useEffect(() => {
    const layerToFadeIn = layers.find((l) => l.opacity === 0);
    if (layerToFadeIn) {
      const timer = setTimeout(() => {
        setLayers((prev) => prev.map((l) => (l.id === layerToFadeIn.id ? { ...l, opacity: 1 } : l)));
        onLoad();
      }, 10);

      return () => clearTimeout(timer);
    }
  }, [layers, onLoad]);

  const handleTransitionEnd = useCallback((finishedId: string) => {
    setLayers((prev) => {
      const finishedIndex = prev.findIndex((l) => l.id === finishedId);
      if (finishedIndex < 0 || prev.length <= 1) {
        return prev;
      }
      return prev.slice(finishedIndex);
    });
  }, []);

  const ringClass = isActive
    ? 'ring-2 ring-accent'
    : isSelected
      ? 'ring-2 ring-gray-400'
      : 'hover:ring-2 hover:ring-hover-color';
  const colorTag = tags?.find((t: string) => t.startsWith('color:'))?.substring(6);
  const colorLabel = COLOR_LABELS.find((c: Color) => c.name === colorTag);

  return (
    <div
      className={`aspect-square bg-surface rounded-md overflow-hidden cursor-pointer group relative transition-all duration-150 ${ringClass}`}
      onClick={(e: React.MouseEvent) => {
        e.stopPropagation();
        onImageClick(path, e);
      }}
      onContextMenu={onContextMenu}
      onDoubleClick={() => onImageDoubleClick(path)}
    >
      {layers.length > 0 && (
        <div className="absolute inset-0 w-full h-full">
          {layers.map((layer) => (
            <div
              key={layer.id}
              className="absolute inset-0 w-full h-full"
              style={{
                opacity: layer.opacity,
                transition: 'opacity 300ms ease-in-out',
              }}
              onTransitionEnd={() => handleTransitionEnd(layer.id)}
            >
              {thumbnailAspectRatio === ThumbnailAspectRatio.Contain && (
                <img
                  alt=""
                  className="absolute inset-0 w-full h-full object-cover blur-md scale-110 brightness-[0.4]"
                  src={layer.url}
                />
              )}
              <img
                alt={path.split(/[\\/]/).pop()}
                className={`w-full h-full group-hover:scale-[1.02] transition-transform duration-300 ${
                  thumbnailAspectRatio === ThumbnailAspectRatio.Contain ? 'object-contain' : 'object-cover'
                } relative`}
                decoding="async"
                loading="lazy"
                src={layer.url}
              />
            </div>
          ))}
        </div>
      )}

      <AnimatePresence>
        {layers.length === 0 && showPlaceholder && (
          <motion.div
            className="absolute inset-0 w-full h-full flex items-center justify-center bg-surface"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.3, ease: 'easeInOut' }}
          >
            <ImageIcon className="text-text-secondary animate-pulse" />
          </motion.div>
        )}
      </AnimatePresence>

      {(colorLabel || rating > 0) && (
        <div className="absolute top-1.5 right-1.5 bg-bg-primary/50 rounded-full px-1.5 py-0.5 flex items-center gap-1 backdrop-blur-sm">
          {colorLabel && (
            <div
              className="w-3 h-3 rounded-full ring-1 ring-black/20"
              style={{ backgroundColor: colorLabel.color }}
              data-tooltip={`Color: ${colorLabel.name}`}
            ></div>
          )}
          {rating > 0 && (
            <>
              <Text variant={TextVariants.label} color={TextColors.primary}>
                {rating}
              </Text>
              <StarIcon size={16} className="text-accent fill-accent" />
            </>
          )}
        </div>
      )}
      <div className="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/70 to-transparent p-2 flex items-end justify-between">
        <Text variant={TextVariants.small} color={TextColors.white} className="truncate pr-2">
          {baseName}
        </Text>
        {isVirtualCopy && (
          <Text
            as="div"
            variant={TextVariants.label}
            color={TextColors.white}
            weight={TextWeights.bold}
            className="flex-shrink-0 bg-bg-primary/50 text-[10px] px-1.5 py-0.5 rounded-full backdrop-blur-sm"
            data-tooltip={t('library.virtualCopy')}
          >
            VC
          </Text>
        )}
      </div>
    </div>
  );
}

type RowItem =
  | { type: 'header'; path: string; count: number }
  | { type: 'images'; images: ImageFile[]; startIndex: number }
  | { type: 'footer' };

interface RowProps {
  index?: number;
  style?: React.CSSProperties;
  rows: RowItem[];
  activePath: string | null;
  multiSelectedPaths: string[];
  onContextMenu: (event: React.MouseEvent, path: string) => void;
  onImageClick: (path: string, event: React.MouseEvent) => void;
  onImageDoubleClick: (path: string) => void;
  thumbnails: Record<string, string>;
  thumbnailAspectRatio: ThumbnailAspectRatio;
  loadedThumbnails: Set<string>;
  imageRatings: Record<string, number>;
  rootPath: string | null;
  itemWidth: number;
  outerPadding: number;
  gap: number;
}

const Row = ({
  index,
  style,
  rows,
  activePath,
  multiSelectedPaths,
  onContextMenu,
  onImageClick,
  onImageDoubleClick,
  thumbnails,
  thumbnailAspectRatio,
  loadedThumbnails,
  imageRatings,
  rootPath,
  itemWidth,
  outerPadding,
  gap,
}: RowProps) => {
  const { t } = useTranslation();
  const row = rows[index!];
  if (row.type === 'footer') return null;
  const shiftedStyle = {
    ...style,
    transform: ((style as React.CSSProperties).transform as string).replace(
      /translateY\(([^)]+)\)/,
      (_: string, y: string) => `translateY(${parseFloat(y) + outerPadding}px)`,
    ),
  };

  if (row.type === 'header') {
    let displayPath = row.path;
    if (rootPath && row.path.startsWith(rootPath)) {
      displayPath = row.path.substring(rootPath.length);
      if (displayPath.startsWith('/') || displayPath.startsWith('\\')) {
        displayPath = displayPath.substring(1);
      }
    }
    if (!displayPath) displayPath = t('library.currentFolder');

    return (
      <div
        style={{
          ...shiftedStyle,
          left: 0,
          width: '100%',
          paddingLeft: outerPadding,
          paddingRight: outerPadding,
          boxSizing: 'border-box',
        }}
        className="flex items-end pb-2"
      >
        <div className="flex items-center gap-2 w-full border-b border-border-color pb-1">
          <FolderOpen size={16} className="text-text-secondary" />
          <Text variant={TextVariants.label} weight={TextWeights.semibold} className="truncate" data-tooltip={row.path}>
            {displayPath}
          </Text>
          <Text variant={TextVariants.small} className="opacity-60 ml-auto">
            {t('library.imagesCount', { count: row.count })}
          </Text>
        </div>
      </div>
    );
  }

  return (
    <div
      style={{
        ...shiftedStyle,
        left: outerPadding,
        right: outerPadding,
        width: 'auto',
        display: 'flex',
        gap: gap,
      }}
    >
      {row.images.map((imageFile: ImageFile) => (
        <div
          key={imageFile.path}
          style={{
            width: itemWidth,
            height: itemWidth,
          }}
        >
          <Thumbnail
            data={thumbnails[imageFile.path]}
            isActive={activePath === imageFile.path}
            isSelected={multiSelectedPaths.includes(imageFile.path)}
            onContextMenu={(e: React.MouseEvent) => onContextMenu(e, imageFile.path)}
            onImageClick={onImageClick}
            onImageDoubleClick={onImageDoubleClick}
            onLoad={() => loadedThumbnails.add(imageFile.path)}
            path={imageFile.path}
            rating={imageRatings?.[imageFile.path] || 0}
            tags={imageFile.tags}
            aspectRatio={thumbnailAspectRatio}
          />
        </div>
      ))}
    </div>
  );
};

export default function MainLibrary({
  activePath,
  aiModelDownloadStatus,
  appSettings,
  currentFolderPath,
  filterCriteria,
  imageList,
  imageRatings,
  importState,
  indexingProgress,
  isIndexing,
  isLoading,
  isThumbnailsLoading,
  isTreeLoading: _isTreeLoading,
  libraryScrollTop,
  libraryViewMode,
  multiSelectedPaths,
  onClearSelection,
  onContextMenu,
  onContinueSession,
  onEmptyAreaContextMenu,
  onGoHome,
  onImageClick,
  onImageDoubleClick,
  onLibraryRefresh,
  onOpenFolder,
  onSettingsChange,
  onThumbnailAspectRatioChange,
  onThumbnailSizeChange,
  rootPath,
  searchCriteria,
  setFilterCriteria,
  setLibraryScrollTop,
  setLibraryViewMode,
  setSearchCriteria,
  setSortCriteria,
  sortCriteria,
  theme,
  thumbnailAspectRatio,
  thumbnails,
  thumbnailSize,
  onNavigateToCommunity,
}: MainLibraryProps) {
  const { t } = useTranslation();
  const [showSettings, setShowSettings] = useState(false);
  const [, setSupportedTypes] = useState<SupportedTypes | null>(null);
  const libraryContainerRef = useRef<HTMLDivElement>(null);
  const [listHandle, setListHandle] = useListCallbackRef();
  const [isLoaderVisible, setIsLoaderVisible] = useState(false);
  const loadedThumbnailsRef = useRef(new Set<string>());

  const prevScrollState = useRef({
    path: null as string | null,
    top: -1,
    folder: null as string | null,
  });

  const groups = useMemo(() => {
    if (libraryViewMode === LibraryViewMode.Flat) return null;
    return groupImagesByFolder(imageList, currentFolderPath);
  }, [imageList, currentFolderPath, libraryViewMode]);

  const handleSortChange = useCallback(
    (criteria: SortCriteria | ((prev: SortCriteria) => SortCriteria)) => {
      onClearSelection();
      setSortCriteria(criteria);
    },
    [onClearSelection, setSortCriteria],
  );

  const sortOptions = useMemo(() => {
    const exifEnabled = appSettings?.enableExifReading ?? false;
    return [
      { key: 'name', label: t('library.fileName') },
      { key: 'date', label: t('library.dateModified') },
      { key: 'rating', label: t('library.rating') },
      { key: 'date_taken', label: t('library.dateTaken'), disabled: !exifEnabled },
      { key: 'focal_length', label: t('library.focalLength'), disabled: !exifEnabled },
      { key: 'iso', label: t('library.iso'), disabled: !exifEnabled },
      { key: 'shutter_speed', label: t('library.shutterSpeed'), disabled: !exifEnabled },
      { key: 'aperture', label: t('library.aperture'), disabled: !exifEnabled },
    ];
  }, [appSettings?.enableExifReading, t]);

  useEffect(() => {
    if (!activePath || !libraryContainerRef.current || multiSelectedPaths.length > 1) return;

    const container = libraryContainerRef.current;
    const width = container.clientWidth;
    const OUTER_PADDING = 12;
    const ITEM_GAP = 12;
    const minThumbWidth = THUMBNAIL_SIZE_VALUES.find((o) => o.id === thumbnailSize)?.size || 240;
    const availableWidth = width - OUTER_PADDING * 2;
    const columnCount = Math.max(1, Math.floor((availableWidth + ITEM_GAP) / (minThumbWidth + ITEM_GAP)));
    const itemWidth = (availableWidth - ITEM_GAP * (columnCount - 1)) / columnCount;
    const rowHeight = itemWidth + ITEM_GAP;
    const headerHeight = 40;

    let targetTop = 0;
    let found = false;

    if (libraryViewMode === LibraryViewMode.Recursive) {
      const groups = groupImagesByFolder(imageList, currentFolderPath);
      for (const group of groups) {
        if (group.images.length === 0) continue;

        targetTop += headerHeight;

        const imageIndex = group.images.findIndex((img) => img.path === activePath);
        if (imageIndex !== -1) {
          const rowIndex = Math.floor(imageIndex / columnCount);
          targetTop += rowIndex * rowHeight;
          found = true;
          break;
        }

        const rowsInGroup = Math.ceil(group.images.length / columnCount);
        targetTop += rowsInGroup * rowHeight;
      }
    } else {
      const index = imageList.findIndex((img) => img.path === activePath);
      if (index !== -1) {
        const rowIndex = Math.floor(index / columnCount);
        targetTop = rowIndex * rowHeight;
        found = true;
      }
    }

    if (found && listHandle?.element) {
      const prev = prevScrollState.current;

      const shouldScroll =
        activePath !== prev.path || Math.abs(targetTop - prev.top) > 1 || currentFolderPath !== prev.folder;

      if (shouldScroll) {
        const element = listHandle.element;
        const clientHeight = element.clientHeight;
        const scrollTop = element.scrollTop;
        const itemBottom = targetTop + rowHeight;
        const SCROLL_OFFSET = 120;

        if (itemBottom > scrollTop + clientHeight) {
          element.scrollTo({
            top: itemBottom - clientHeight + SCROLL_OFFSET,
            behavior: 'smooth',
          });
        } else if (targetTop < scrollTop) {
          element.scrollTo({
            top: targetTop - SCROLL_OFFSET,
            behavior: 'smooth',
          });
        }

        prevScrollState.current = {
          path: activePath,
          top: targetTop,
          folder: currentFolderPath,
        };
      }
    }
  }, [activePath, imageList, libraryViewMode, thumbnailSize, currentFolderPath, multiSelectedPaths.length, listHandle]);

  useEffect(() => {
    if (listHandle?.element && libraryScrollTop > 0) {
      listHandle.element.scrollTop = libraryScrollTop;
    }
  }, [listHandle]);

  useEffect(() => {
    const exifEnabled = appSettings?.enableExifReading ?? true;
    const exifSortKeys = ['date_taken', 'iso', 'shutter_speed', 'aperture', 'focal_length'];
    const isCurrentSortExif = exifSortKeys.includes(sortCriteria.key);

    if (!exifEnabled && isCurrentSortExif) {
      setSortCriteria({ key: 'name', order: SortDirection.Ascending });
    }
  }, [appSettings?.enableExifReading, sortCriteria.key, setSortCriteria]);

  useEffect(() => {
    let showTimer: number | undefined;
    let hideTimer: number | undefined;

    if (isThumbnailsLoading || isLoading) {
      showTimer = window.setTimeout(() => {
        setIsLoaderVisible(true);
      }, 1000);
    } else {
      hideTimer = window.setTimeout(() => {
        setIsLoaderVisible(false);
      }, 500);
    }
    return () => {
      clearTimeout(showTimer);
      clearTimeout(hideTimer);
    };
  }, [isThumbnailsLoading, isLoading]);

  useEffect(() => {
    invoke(Invokes.GetSupportedFileTypes)
      .then((types) => setSupportedTypes(types as SupportedTypes))
      .catch((err) => console.error('Failed to load supported file types:', err));
  }, []);

  useEffect(() => {
    const handleWheel = (event: WheelEvent) => {
      const container = libraryContainerRef.current;
      if (!container || !container.contains(event.target as Node)) {
        return;
      }

      if (event.ctrlKey || event.metaKey) {
        event.preventDefault();
        const currentIndex = THUMBNAIL_SIZE_VALUES.findIndex((o) => o.id === thumbnailSize);
        if (currentIndex === -1) {
          return;
        }

        const nextIndex =
          event.deltaY < 0
            ? Math.min(currentIndex + 1, THUMBNAIL_SIZE_VALUES.length - 1)
            : Math.max(currentIndex - 1, 0);
        if (nextIndex !== currentIndex) {
          onThumbnailSizeChange(THUMBNAIL_SIZE_VALUES[nextIndex].id);
        }
      }
    };

    window.addEventListener('wheel', handleWheel, { passive: false });
    return () => {
      window.removeEventListener('wheel', handleWheel);
    };
  }, [thumbnailSize, onThumbnailSizeChange]);

  if (!rootPath) {
    if (!appSettings) {
      return;
    }
    const hasLastPath = !!appSettings.lastRootPath;
    const currentThemeId = theme || DEFAULT_THEME_ID;
    const selectedTheme: ThemeProps | undefined =
      THEMES.find((t: ThemeProps) => t.id === currentThemeId) ||
      THEMES.find((t: ThemeProps) => t.id === DEFAULT_THEME_ID);
    const splashImage = selectedTheme?.splashImage;
    return (
      <div className={`flex-1 flex h-full bg-bg-secondary overflow-hidden shadow-lg`}>
        <div className="w-1/2 hidden md:block relative">
          <AnimatePresence>
            <motion.img
              alt="Splash screen background"
              animate={{ opacity: 1 }}
              className="absolute inset-0 w-full h-full object-cover"
              exit={{ opacity: 0 }}
              initial={{ opacity: 0 }}
              key={splashImage}
              src={splashImage}
              transition={{ duration: 0.5, ease: 'easeInOut' }}
            />
          </AnimatePresence>
        </div>
        <div className="w-full md:w-1/2 flex flex-col p-8 lg:p-16 relative">
          {showSettings ? (
            <SettingsPanel
              appSettings={appSettings ?? { lastRootPath: null, theme: 'dark' as const }}
              onBack={() => setShowSettings(false)}
              onLibraryRefresh={onLibraryRefresh}
              onSettingsChange={onSettingsChange}
              rootPath={rootPath}
            />
          ) : (
            <>
              <div className="my-auto text-left">
                <Text variant={TextVariants.displayLarge}>QRaw</Text>
                <Text
                  variant={TextVariants.heading}
                  color={TextColors.secondary}
                  weight={TextWeights.normal}
                  className="mb-10 max-w-md"
                >
                  {hasLastPath ? (
                    <>
                      {t('library.welcomeBack')}
                      <br />
                      {t('library.continueOrNew')}
                    </>
                  ) : (
                    t('library.welcomeDescription')
                  )}
                </Text>
                <div className="flex flex-col w-full max-w-xs gap-3">
                  {hasLastPath && (
                    <Button
                      className="rounded-lg h-11 w-full flex justify-center items-center gap-2 font-medium"
                      onClick={onContinueSession}
                      size="lg"
                    >
                      <RefreshCw size={18} /> {t('library.continueSession')}
                    </Button>
                  )}
                  <div className="flex items-center gap-2">
                    <Button
                      className={`rounded-lg flex-grow flex justify-center items-center gap-2 h-11 font-medium ${
                        hasLastPath ? 'bg-surface text-text-primary shadow-none' : ''
                      }`}
                      onClick={onOpenFolder}
                      size="lg"
                    >
                      <Folder size={18} />
                      {hasLastPath ? t('library.changeFolder') : t('library.openFolder')}
                    </Button>
                    <Button
                      className="w-11 h-11 flex items-center justify-center bg-surface text-text-primary shadow-none rounded-lg hover:bg-card-active transition-colors"
                      onClick={() => setShowSettings(true)}
                      size="icon"
                      data-tooltip={t('settings.title')}
                      variant="ghost"
                    >
                      <Settings size={18} />
                    </Button>
                  </div>
                </div>
              </div>
              <Text
                variant={TextVariants.small}
                as="div"
                className="absolute bottom-8 left-8 lg:left-16 space-y-1 text-text-secondary"
              >
                <p>
                  <a
                    href="https://instagram.com/timonkaech.photography"
                    className="hover:underline"
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    @Q
                  </a>
                  <span className="ml-[3px]">作品</span>
                  <span className="ml-[10px]">{t('library.imagesBy')} </span>
                </p>
                <p>{t('library.version', { version: '1.0.0' })}</p>
              </Text>
            </>
          )}
        </div>
      </div>
    );
  }

  return (
    <div
      className="flex-1 flex flex-col h-full min-w-0 bg-bg-secondary rounded-lg overflow-hidden"
      ref={libraryContainerRef}
    >
      <header className="p-4 flex-shrink-0 flex justify-between items-center border-b border-border-color gap-4">
        <div className="min-w-0">
          <Text variant={TextVariants.headline}>{t('library.title')}</Text>
          <div className="flex items-center gap-2">
            {currentFolderPath ? (
              <Text className="truncate">{currentFolderPath}</Text>
            ) : (
              <p className="text-sm invisible select-none pointer-events-none h-5 overflow-hidden"></p>
            )}
            <div
              className={`overflow-hidden transition-all duration-300 ${
                isLoaderVisible ? 'max-w-[1rem] opacity-100' : 'max-w-0 opacity-0'
              }`}
            >
              <Loader2 size={14} className="animate-spin text-text-secondary" />
            </div>
          </div>
        </div>
        <div className="flex items-center gap-3 flex-shrink-0">
          {importState.status === Status.Importing && (
            <Text as="div" color={TextColors.accent} className="flex items-center gap-2 animate-pulse">
              <FolderInput size={16} />
              <span>
                {t('library.importing', { current: importState.progress?.current, total: importState.progress?.total })}
              </span>
            </Text>
          )}
          {importState.status === Status.Success && (
            <Text as="div" color={TextColors.success} className="flex items-center gap-2">
              <Check size={16} />
              <span>{t('library.importComplete')}</span>
            </Text>
          )}
          {importState.status === Status.Error && (
            <Text as="div" color={TextColors.error} className="flex items-center gap-2">
              <AlertTriangle size={16} />
              <span>{t('library.importFailed')}</span>
            </Text>
          )}
          <SearchInput
            indexingProgress={indexingProgress}
            isIndexing={isIndexing}
            searchCriteria={searchCriteria}
            setSearchCriteria={setSearchCriteria}
          />
          <ViewOptionsDropdown
            filterCriteria={filterCriteria}
            libraryViewMode={libraryViewMode}
            onSelectSize={onThumbnailSizeChange}
            onSelectAspectRatio={onThumbnailAspectRatioChange}
            setFilterCriteria={setFilterCriteria}
            setLibraryViewMode={setLibraryViewMode}
            setSortCriteria={handleSortChange}
            sortCriteria={sortCriteria}
            sortOptions={sortOptions}
            thumbnailSize={thumbnailSize}
            thumbnailAspectRatio={thumbnailAspectRatio}
          />
          <Button
            className="h-12 w-12 bg-surface text-text-primary shadow-none p-0 flex items-center justify-center rounded-lg hover:bg-card-active transition-colors"
            onClick={onNavigateToCommunity}
            data-tooltip={t('library.communityPresets')}
          >
            <Users size={20} />
          </Button>
          <Button
            className="h-12 w-12 bg-surface text-text-primary shadow-none p-0 flex items-center justify-center rounded-lg hover:bg-card-active transition-colors"
            onClick={onOpenFolder}
            data-tooltip={t('library.openAnotherFolder')}
          >
            <Folder size={20} />
          </Button>
          <Button
            className="h-12 w-12 bg-surface text-text-primary shadow-none p-0 flex items-center justify-center rounded-lg hover:bg-card-active transition-colors"
            onClick={onGoHome}
            data-tooltip={t('common.goToHome')}
          >
            <Home size={20} />
          </Button>
        </div>
      </header>
      {imageList.length > 0 ? (
        <div className="flex-1 w-full h-full" onClick={onClearSelection} onContextMenu={onEmptyAreaContextMenu}>
          <AutoSizer>
            {({ height, width }) => {
              const OUTER_PADDING = 12;
              const ITEM_GAP = 12;
              const minThumbWidth = THUMBNAIL_SIZE_VALUES.find((o) => o.id === thumbnailSize)?.size || 240;

              const availableWidth = width - OUTER_PADDING * 2;
              const columnCount = Math.max(1, Math.floor((availableWidth + ITEM_GAP) / (minThumbWidth + ITEM_GAP)));
              const itemWidth = (availableWidth - ITEM_GAP * (columnCount - 1)) / columnCount;
              const rowHeight = itemWidth + ITEM_GAP;
              const headerHeight = 40;

              const rows: RowItem[] = [];

              if (libraryViewMode === LibraryViewMode.Recursive && groups) {
                groups.forEach((group) => {
                  if (group.images.length === 0) return;

                  rows.push({ type: 'header', path: group.path, count: group.images.length });

                  for (let i = 0; i < group.images.length; i += columnCount) {
                    rows.push({
                      type: 'images',
                      images: group.images.slice(i, i + columnCount),
                      startIndex: i,
                    });
                  }
                });
              } else {
                for (let i = 0; i < imageList.length; i += columnCount) {
                  rows.push({
                    type: 'images',
                    images: imageList.slice(i, i + columnCount),
                    startIndex: i,
                  });
                }
              }

              rows.push({ type: 'footer' });

              const getItemSize = (index: number) => {
                if (rows[index].type === 'footer') return OUTER_PADDING;
                return rows[index].type === 'header' ? headerHeight : rowHeight;
              };

              return (
                <div key={`${width}-${thumbnailSize}-${libraryViewMode}`} style={{ height, width }}>
                  <List
                    listRef={setListHandle}
                    rowCount={rows.length}
                    rowHeight={getItemSize}
                    onScroll={(e: React.UIEvent<HTMLElement>) => setLibraryScrollTop(e.currentTarget.scrollTop)}
                    className="custom-scrollbar"
                    rowComponent={Row}
                    rowProps={{
                      rows,
                      activePath,
                      multiSelectedPaths,
                      onContextMenu,
                      onImageClick,
                      onImageDoubleClick,
                      thumbnails,
                      thumbnailAspectRatio,
                      loadedThumbnails: loadedThumbnailsRef.current,
                      imageRatings,
                      rootPath: currentFolderPath,
                      itemWidth,
                      outerPadding: OUTER_PADDING,
                      gap: ITEM_GAP,
                    }}
                  />
                </div>
              );
            }}
          </AutoSizer>
        </div>
      ) : isIndexing || aiModelDownloadStatus || importState.status === Status.Importing ? (
        <div className="flex-1 flex flex-col items-center justify-center" onContextMenu={onEmptyAreaContextMenu}>
          <Loader2 className="h-12 w-12 text-secondary animate-spin mb-4" />
          <Text variant={TextVariants.heading} color={TextColors.secondary}>
            {aiModelDownloadStatus
              ? t('library.downloadingModel', { model: aiModelDownloadStatus })
              : isIndexing && indexingProgress.total > 0
                ? t('library.indexingProgress', { current: indexingProgress.current, total: indexingProgress.total })
                : importState.status === Status.Importing &&
                    importState?.progress?.total &&
                    importState.progress.total > 0
                  ? t('library.importingProgress', {
                      current: importState.progress?.current,
                      total: importState.progress?.total,
                    })
                  : t('library.processingImages')}
          </Text>
          <Text className="mt-2">{t('library.thisMayTakeAMoment')}</Text>
        </div>
      ) : searchCriteria.tags.length > 0 || searchCriteria.text ? (
        <div
          className="flex-1 flex flex-col items-center justify-center text-text-secondary text-center"
          onContextMenu={onEmptyAreaContextMenu}
        >
          <Search className="h-12 w-12 text-secondary mb-4" />
          <Text variant={TextVariants.heading} color={TextColors.secondary}>
            {t('library.noResultsFound')}
          </Text>
          <Text className="mt-2 max-w-sm">
            {t('library.noResultsDescription')}
            {!appSettings?.enableAiTagging && ' ' + t('library.enableTaggingHint')}
          </Text>
        </div>
      ) : (
        <div className="flex-1 flex flex-col items-center justify-center" onContextMenu={onEmptyAreaContextMenu}>
          <SlidersHorizontal className="h-12 w-12 mb-4 text-text-secondary" />
          <Text>{t('library.noImagesMatchFilter')}</Text>
        </div>
      )}
    </div>
  );
}
