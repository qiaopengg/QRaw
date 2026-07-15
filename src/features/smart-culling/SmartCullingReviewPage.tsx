import { useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { ArrowLeft, CheckCircle, Star, Tag, Trash2 } from 'lucide-react';
import Button from '../../components/ui/Button';
import Dropdown from '../../components/ui/Dropdown';
import Text from '../../components/ui/Text';
import { TextColors, TextVariants } from '../../types/typography';
import { useProcessStore } from '../../store/useProcessStore';
import { Invokes } from '../../components/ui/AppProperties';
import type { LibraryFeatureViewSlotProps } from '../contracts';
import { SMART_CULLING_INVOKES } from './constants';
import { useSmartCullingStore } from './useSmartCulling';
import type { ImageAnalysisResult, SmartCullingApplyAction } from './types';

function ImageThumbnail({
  path,
  thumbnails,
  isSelected,
  onToggle,
  children,
}: {
  path: string;
  thumbnails: Record<string, string>;
  isSelected: boolean;
  onToggle(): void;
  children?: React.ReactNode;
}) {
  const thumbnailUrl = thumbnails[path];
  return (
    <div
      className={`relative group rounded-md overflow-hidden border-2 transition-colors cursor-pointer ${
        isSelected ? 'border-accent' : 'border-transparent hover:border-surface'
      }`}
      onClick={onToggle}
    >
      <img
        src={thumbnailUrl}
        alt={path}
        className={`w-full h-full object-cover transition-opacity ${isSelected ? 'opacity-100' : 'opacity-75 group-hover:opacity-100'}`}
      />
      <div
        className={`absolute inset-0 bg-black/50 transition-opacity ${
          isSelected ? 'opacity-0' : 'opacity-100 group-hover:opacity-0'
        }`}
      />
      <div className="absolute top-2 right-2">{isSelected && <CheckCircle size={16} className="text-accent" />}</div>
      {children && (
        <Text
          as="div"
          variant={TextVariants.small}
          color={TextColors.white}
          className="absolute bottom-0 left-0 right-0 p-1 bg-black/60"
        >
          {children}
        </Text>
      )}
    </div>
  );
}

export default function SmartCullingReviewPage({ onBackToLibrary, onLibraryRefresh }: LibraryFeatureViewSlotProps) {
  const { t } = useTranslation();
  const { suggestions, setSmartCulling } = useSmartCullingStore();
  const thumbnails = useProcessStore((state) => state.thumbnails);

  const [action, setAction] = useState<SmartCullingApplyAction>('reject');
  const [activeTab, setActiveTab] = useState<'similar' | 'blurry' | 'faces'>('similar');
  const [isApplying, setIsApplying] = useState(false);

  const numSimilar = suggestions?.similarGroups.reduce((acc, group) => acc + group.duplicates.length, 0) || 0;
  const numBlurry = suggestions?.blurryImages.length || 0;
  const numFaces = suggestions?.problemFaces.length || 0;

  const initialRejects = useMemo(() => {
    const set = new Set<string>();
    suggestions?.similarGroups.forEach((group) => group.duplicates.forEach((dup) => set.add(dup.path)));
    suggestions?.blurryImages.forEach((img) => set.add(img.path));
    return set;
  }, [suggestions]);

  const [selectedRejects, setSelectedRejects] = useState<Set<string>>(initialRejects);

  const handleToggleReject = (path: string) => {
    setSelectedRejects((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(path)) newSet.delete(path);
      else newSet.add(path);
      return newSet;
    });
  };

  const reasonFor = (result: ImageAnalysisResult): string => {
    if (result.faces.some((face) => face.isClosed)) return 'closed_eyes';
    if (suggestions?.blurryImages.some((img) => img.path === result.path)) return 'blurry';
    return 'similar_group_duplicate';
  };

  const handleClose = () => {
    setSmartCulling({ suggestions: null, error: null });
    onBackToLibrary();
  };

  const handleApply = async () => {
    if (!suggestions || selectedRejects.size === 0) return;
    setIsApplying(true);
    const paths = Array.from(selectedRejects);

    const allResults = [
      ...suggestions.similarGroups.flatMap((group) => group.duplicates),
      ...suggestions.blurryImages,
      ...suggestions.problemFaces,
    ];
    const resultByPath = new Map(allResults.map((result) => [result.path, result]));

    try {
      await invoke(SMART_CULLING_INVOKES.WriteMetadata, {
        items: paths.map((path) => {
          const result = resultByPath.get(path);
          return {
            path,
            score: result?.qualityScore ?? 0,
            reasonText: result ? reasonFor(result) : 'flagged',
            status: 'reject_suggestion',
          };
        }),
      });

      if (action === 'reject') {
        await invoke(Invokes.SetColorLabelForPaths, { paths, color: 'red' });
      } else if (action === 'rate_zero') {
        await invoke(Invokes.SetRatingForPaths, { paths, rating: 0 });
      } else if (action === 'delete') {
        await invoke('delete_files_from_disk', { paths });
      }

      await onLibraryRefresh?.();
      handleClose();
    } catch (err) {
      console.error('Failed to apply smart culling result:', err);
    } finally {
      setIsApplying(false);
    }
  };

  if (!suggestions) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-4">
        <Text variant={TextVariants.heading}>{t('modals.smartCulling.noIssuesFound')}</Text>
        <Button onClick={onBackToLibrary}>{t('modals.smartCulling.close')}</Button>
      </div>
    );
  }

  const totalSuggestions = numSimilar + numBlurry + numFaces;

  return (
    <div className="flex-1 flex flex-col h-full min-w-0 bg-bg-secondary rounded-lg overflow-hidden">
      <header className="p-4 border-b border-border-color flex items-center justify-between gap-4">
        <div className="flex items-center gap-3 min-w-0">
          <Button className="bg-surface text-text-primary h-10 w-10 p-0" onClick={handleClose}>
            <ArrowLeft size={18} />
          </Button>
          <Text variant={TextVariants.headline}>{t('modals.smartCulling.cullingSuggestions')}</Text>
        </div>
      </header>

      {totalSuggestions === 0 ? (
        <div className="flex-1 flex flex-col items-center justify-center">
          <CheckCircle className="w-16 h-16 text-green-500" />
          <Text variant={TextVariants.heading} className="mt-4">
            {t('modals.smartCulling.noIssuesFound')}
          </Text>
          <Text>{t('modals.smartCulling.noIssuesDesc')}</Text>
          <div className="mt-6">
            <Button onClick={handleClose}>{t('modals.smartCulling.done')}</Button>
          </div>
        </div>
      ) : (
        <>
          <div className="px-4 pt-4 border-b border-border-color">
            <nav className="-mb-px flex space-x-4" aria-label="Tabs">
              {numSimilar > 0 && (
                <button
                  onClick={() => setActiveTab('similar')}
                  className={`${
                    activeTab === 'similar'
                      ? 'border-accent text-accent'
                      : 'border-transparent text-text-secondary hover:text-text-primary hover:border-gray-300'
                  } whitespace-nowrap py-2 px-1 border-b-2 font-medium text-sm`}
                >
                  {t('modals.smartCulling.similarGroupsTab')}{' '}
                  <span className="bg-surface text-text-secondary rounded-full px-2 py-0.5 text-xs">{numSimilar}</span>
                </button>
              )}
              {numBlurry > 0 && (
                <button
                  onClick={() => setActiveTab('blurry')}
                  className={`${
                    activeTab === 'blurry'
                      ? 'border-accent text-accent'
                      : 'border-transparent text-text-secondary hover:text-text-primary hover:border-gray-300'
                  } whitespace-nowrap py-2 px-1 border-b-2 font-medium text-sm`}
                >
                  {t('modals.smartCulling.blurryImagesTab')}{' '}
                  <span className="bg-surface text-text-secondary rounded-full px-2 py-0.5 text-xs">{numBlurry}</span>
                </button>
              )}
              {numFaces > 0 && (
                <button
                  onClick={() => setActiveTab('faces')}
                  className={`${
                    activeTab === 'faces'
                      ? 'border-accent text-accent'
                      : 'border-transparent text-text-secondary hover:text-text-primary hover:border-gray-300'
                  } whitespace-nowrap py-2 px-1 border-b-2 font-medium text-sm`}
                >
                  {t('modals.smartCulling.problemFacesTab')}{' '}
                  <span className="bg-surface text-text-secondary rounded-full px-2 py-0.5 text-xs">{numFaces}</span>
                </button>
              )}
            </nav>
          </div>

          <div className="flex-1 overflow-y-auto custom-scrollbar p-4">
            {activeTab === 'similar' && (
              <div className="space-y-4">
                {suggestions.similarGroups.map((group, index) => (
                  <div key={index} className="bg-surface rounded-lg p-3">
                    <Text variant={TextVariants.heading} className="mb-2">
                      {t('modals.smartCulling.groupHeader', { index: index + 1 })}
                    </Text>
                    <div className="grid grid-cols-[1fr_3fr] gap-3">
                      <div>
                        <Text variant={TextVariants.label} className="mb-1">
                          {t('modals.smartCulling.bestImage')}
                        </Text>
                        <div className="relative rounded-md overflow-hidden border-2 border-green-500">
                          <img
                            src={thumbnails[group.representative.path]}
                            alt="Representative"
                            className="w-full h-full object-cover"
                          />
                          <Text
                            as="div"
                            variant={TextVariants.small}
                            color={TextColors.white}
                            className="absolute bottom-0 left-0 right-0 p-1 bg-black/60"
                          >
                            {t('modals.smartCulling.score', { score: group.representative.qualityScore.toFixed(2) })}
                          </Text>
                        </div>
                      </div>
                      <div>
                        <Text variant={TextVariants.label} className="mb-1">
                          {t('modals.smartCulling.duplicatesHeader', { count: group.duplicates.length })}
                        </Text>
                        <div className="grid grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-2">
                          {group.duplicates.map((dup) => (
                            <ImageThumbnail
                              key={dup.path}
                              path={dup.path}
                              thumbnails={thumbnails}
                              isSelected={selectedRejects.has(dup.path)}
                              onToggle={() => handleToggleReject(dup.path)}
                            >
                              {t('modals.smartCulling.score', { score: dup.qualityScore.toFixed(2) })}
                            </ImageThumbnail>
                          ))}
                        </div>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            )}
            {activeTab === 'blurry' && (
              <div className="grid grid-cols-4 md:grid-cols-5 lg:grid-cols-6 gap-2">
                {suggestions.blurryImages.map((img) => (
                  <ImageThumbnail
                    key={img.path}
                    path={img.path}
                    thumbnails={thumbnails}
                    isSelected={selectedRejects.has(img.path)}
                    onToggle={() => handleToggleReject(img.path)}
                  >
                    {t('modals.smartCulling.sharpness', { sharpness: img.sharpnessMetric.toFixed(0) })}
                  </ImageThumbnail>
                ))}
              </div>
            )}
            {activeTab === 'faces' && (
              <div className="grid grid-cols-4 md:grid-cols-5 lg:grid-cols-6 gap-2">
                {suggestions.problemFaces.map((img) => (
                  <ImageThumbnail
                    key={img.path}
                    path={img.path}
                    thumbnails={thumbnails}
                    isSelected={selectedRejects.has(img.path)}
                    onToggle={() => handleToggleReject(img.path)}
                  >
                    {t('modals.smartCulling.closedEyesCount', {
                      count: img.faces.filter((face) => face.isClosed).length,
                    })}
                  </ImageThumbnail>
                ))}
              </div>
            )}
          </div>

          <div className="p-4 border-t border-border-color flex justify-between items-center gap-3">
            <div className="w-56">
              <Dropdown
                value={action}
                onChange={(newValue: SmartCullingApplyAction) => setAction(newValue)}
                options={[
                  { value: 'reject', label: t('modals.smartCulling.actionReject') },
                  { value: 'rate_zero', label: t('modals.smartCulling.actionRateZero') },
                  { value: 'delete', label: t('modals.smartCulling.actionDelete') },
                ]}
              />
            </div>
            <Button onClick={handleApply} disabled={selectedRejects.size === 0 || isApplying}>
              {action === 'delete' ? (
                <Trash2 size={16} />
              ) : action === 'rate_zero' ? (
                <Star size={16} />
              ) : (
                <Tag size={16} />
              )}
              {t('modals.smartCulling.applyButton', { count: selectedRejects.size })}
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
