import { useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { homeDir } from '@tauri-apps/api/path';
import { useLibraryStore } from '../store/useLibraryStore';
import { useEditorStore } from '../store/useEditorStore';
import { useUIStore } from '../store/useUIStore';
import { useProcessStore } from '../store/useProcessStore';
import { useSettingsStore } from '../store/useSettingsStore';
import { Invokes, LibraryViewMode, ImageFile } from '../components/ui/AppProperties';
import { INITIAL_ADJUSTMENTS, normalizeLoadedAdjustments } from '../utils/adjustments';

export interface AppNavigationProps {
  setError: (msg: string | null) => void;
  clearThumbnailQueue: () => void;
  debouncedSave: any;
  debouncedSetHistory: any;
  refs: {
    imageCacheRef: React.RefObject<any>;
    transformWrapperRef: React.RefObject<any>;
    preloadedDataRef: React.RefObject<any>;
    cachedEditStateRef: React.RefObject<any>;
    selectedImagePathRef: React.RefObject<string | null>;
    isBackendReadyRef: React.RefObject<boolean>;
    latestRenderedJobIdRef: React.RefObject<number>;
    previewJobIdRef: React.RefObject<number>;
    currentResRef: React.RefObject<number>;
    prevAdjustmentsRef: React.RefObject<any>;
  };
}

export function useAppNavigation({
  setError,
  clearThumbnailQueue,
  debouncedSave,
  debouncedSetHistory,
  refs,
}: AppNavigationProps) {
  const {
    imageCacheRef,
    transformWrapperRef,
    preloadedDataRef,
    cachedEditStateRef,
    selectedImagePathRef,
    isBackendReadyRef,
    latestRenderedJobIdRef,
    previewJobIdRef,
    currentResRef,
    prevAdjustmentsRef,
  } = refs;

  const setLibrary = useLibraryStore((state) => state.setLibrary);
  const setEditor = useEditorStore((state) => state.setEditor);
  const setUI = useUIStore((state) => state.setUI);
  const setProcess = useProcessStore((state) => state.setProcess);
  const handleSettingsChange = useSettingsStore((state) => state.handleSettingsChange);

  const appSettings = useSettingsStore((state) => state.appSettings);
  const osPlatform = useSettingsStore((state) => state.osPlatform);
  const isAndroid = osPlatform === 'android';

  const rootPath = useLibraryStore((state) => state.rootPath);
  const currentFolderPath = useLibraryStore((state) => state.currentFolderPath);
  const pinnedFolders = appSettings?.pinnedFolders || [];
  const libraryViewMode = appSettings?.libraryViewMode;
  const sortCriteria = useLibraryStore((state) => state.sortCriteria);
  const selectedImage = useEditorStore((state) => state.selectedImage);
  const isSliderDragging = useEditorStore((state) => state.isSliderDragging);
  const resetHistory = useEditorStore((state) => state.resetHistory);

  const handleGoHome = useCallback(() => {
    setLibrary({
      rootPath: null,
      currentFolderPath: null,
      imageList: [],
      imageRatings: {},
      folderTree: null,
      multiSelectedPaths: [],
      libraryActivePath: null,
      expandedFolders: new Set(),
    });
    setUI({ isLibraryExportPanelVisible: false });
  }, [setLibrary, setUI]);

  const handleBackToLibrary = useCallback(() => {
    if (selectedImage?.path && cachedEditStateRef.current) {
      imageCacheRef.current.set(selectedImage.path, cachedEditStateRef.current);
    }
    if (transformWrapperRef.current) {
      transformWrapperRef.current.resetTransform(0);
    }
    setEditor({ zoom: 1 });

    debouncedSave.flush();
    debouncedSetHistory.cancel();

    const lastActivePath = selectedImage?.path ?? null;

    setEditor({
      hasRenderedFirstFrame: false,
      selectedImage: null,
      finalPreviewUrl: null,
      uncroppedAdjustedPreviewUrl: null,
      histogram: null,
      waveform: null,
      activeMaskId: null,
      activeMaskContainerId: null,
      activeAiPatchContainerId: null,
      isWbPickerActive: false,
      activeAiSubMaskId: null,
      transformedOriginalUrl: null,
    });

    selectedImagePathRef.current = null;

    setLibrary({ libraryActivePath: lastActivePath });
    setUI({ slideDirection: 1 });

    setEditor({ adjustments: INITIAL_ADJUSTMENTS });
    resetHistory(INITIAL_ADJUSTMENTS);

    isBackendReadyRef.current = true;
    setEditor((state) => {
      if (state.interactivePatch?.url) URL.revokeObjectURL(state.interactivePatch.url);
      return { interactivePatch: null };
    });
  }, [selectedImage?.path, resetHistory, debouncedSave, debouncedSetHistory, setUI, setLibrary, setEditor, refs]);

  const handleImageSelect = useCallback(
    async (path: string) => {
      if (selectedImage?.path === path) return;

      debouncedSave.flush();
      debouncedSetHistory.cancel();

      if (selectedImage?.path && cachedEditStateRef.current) {
        imageCacheRef.current.set(selectedImage.path, cachedEditStateRef.current);
      }

      const cached = imageCacheRef.current.get(path);
      const isFrontendCached = Boolean(cached && cached.selectedImage?.isReady);
      const isCachedInBackend = isFrontendCached
        ? await invoke<boolean>('is_image_cached', { path }).catch(() => false)
        : false;

      const hasDifferentResolution =
        cached &&
        (useEditorStore.getState().originalSize.width !== cached.originalSize.width ||
          useEditorStore.getState().originalSize.height !== cached.originalSize.height);

      if (!isCachedInBackend || hasDifferentResolution) {
        setEditor({ hasRenderedFirstFrame: false });
      }

      selectedImagePathRef.current = path;
      setLibrary({ multiSelectedPaths: [path], libraryActivePath: null, selectionAnchorPath: path });
      setError(null);

      setEditor({
        showOriginal: false,
        activeMaskId: null,
        activeMaskContainerId: null,
        activeAiPatchContainerId: null,
        activeAiSubMaskId: null,
        isWbPickerActive: false,
        transformedOriginalUrl: null,
      });

      setUI({
        isLibraryExportPanelVisible: false,
        compactEditorPanelHeightOverride: null,
      });

      if (isFrontendCached) {
        setEditor({
          selectedImage: {
            ...cached.selectedImage,
            thumbnailUrl: useProcessStore.getState().thumbnails[path] || cached.selectedImage.thumbnailUrl,
          },
          originalSize: cached.originalSize,
          previewSize: cached.previewSize,
          histogram: cached.histogram,
          waveform: cached.waveform,
          finalPreviewUrl: cached.finalPreviewUrl,
          uncroppedAdjustedPreviewUrl: cached.uncroppedPreviewUrl,
        });

        setEditor({ adjustments: cached.adjustments });
        resetHistory(cached.adjustments);
        prevAdjustmentsRef.current = { path, adjustments: cached.adjustments };

        setLibrary({ isViewLoading: false });

        latestRenderedJobIdRef.current = previewJobIdRef.current;
        isBackendReadyRef.current = false;
        currentResRef.current = Infinity;

        invoke(Invokes.LoadImage, { path })
          .then((_result: any) => {
            if (selectedImagePathRef.current !== path) return;
            isBackendReadyRef.current = true;
            currentResRef.current = 0;
            setEditor({ originalSize: { width: _result.width, height: _result.height } });
          })
          .catch((err: any) => {
            if (String(err).includes('cancelled')) return;
            console.error('Background load_image failed on cache hit:', err);
            isBackendReadyRef.current = true;
            currentResRef.current = 0;
          });

        invoke(Invokes.LoadMetadata, { path })
          .then((metadata: any) => {
            if (selectedImagePathRef.current !== path) return;
            let freshAdjustments: any;
            if (metadata.adjustments && !metadata.adjustments.is_null) {
              freshAdjustments = normalizeLoadedAdjustments(metadata.adjustments);
            } else {
              freshAdjustments = { ...INITIAL_ADJUSTMENTS };
            }
            if (!isSliderDragging && JSON.stringify(cached.adjustments) !== JSON.stringify(freshAdjustments)) {
              setEditor({ adjustments: freshAdjustments });
              resetHistory(freshAdjustments);
              prevAdjustmentsRef.current = { path, adjustments: freshAdjustments };
              imageCacheRef.current.set(path, { ...cached, adjustments: freshAdjustments });
            }
          })
          .catch((err) => console.error('Failed background metadata sync on cache hit:', err));

        return;
      }

      isBackendReadyRef.current = true;

      setEditor({
        selectedImage: {
          exif: null,
          height: 0,
          isRaw: false,
          isReady: false,
          metadata: null,
          originalUrl: null,
          path,
          thumbnailUrl: useProcessStore.getState().thumbnails[path],
          width: 0,
        },
        originalSize: { width: 0, height: 0 },
        previewSize: { width: 0, height: 0 },
        histogram: null,
        waveform: null,
        uncroppedAdjustedPreviewUrl: null,
      });

      setLibrary({ isViewLoading: true });

      setEditor((state) => {
        const prev = state.finalPreviewUrl;
        if (prev?.startsWith('blob:') && !imageCacheRef.current.isProtected(prev)) {
          setTimeout(() => {
            if (!imageCacheRef.current.isProtected(prev)) {
              URL.revokeObjectURL(prev);
            }
          }, 250);
        }
        return { finalPreviewUrl: null };
      });

      setEditor((state) => {
        if (state.interactivePatch?.url) URL.revokeObjectURL(state.interactivePatch.url);
        return { interactivePatch: null };
      });
    },
    [
      selectedImage?.path,
      debouncedSave,
      debouncedSetHistory,
      resetHistory,
      isSliderDragging,
      setUI,
      setLibrary,
      setEditor,
      refs,
      setError,
    ],
  );

  const handleSelectSubfolder = useCallback(
    async (path: string | null, isNewRoot = false, preloadedImages?: ImageFile[], expandParents = true) => {
      await invoke('cancel_thumbnail_generation');
      clearThumbnailQueue();
      setLibrary({ isViewLoading: true });
      useLibraryStore.getState().setSearchCriteria({ tags: [], text: '', mode: 'OR' });
      setLibrary({ libraryScrollTop: 0 });
      setProcess({ thumbnails: {} });
      imageCacheRef.current.clear();

      try {
        setLibrary({ currentFolderPath: path });
        setUI({ activeView: 'library' });

        if (isNewRoot) {
          if (path) {
            setLibrary({ expandedFolders: new Set([path]) });
          }
        } else if (path && expandParents) {
          setLibrary((state) => {
            const newSet = new Set(state.expandedFolders);
            const allRoots = [state.rootPath, ...pinnedFolders].filter(Boolean) as string[];
            const relevantRoot = allRoots.find((r) => path.startsWith(r));

            if (relevantRoot) {
              const separator = path.includes('/') ? '/' : '\\';
              const parentSeparatorIndex = path.lastIndexOf(separator);

              if (parentSeparatorIndex > -1 && path.length > relevantRoot.length) {
                let current = path.substring(0, parentSeparatorIndex);
                while (current && current.length >= relevantRoot.length) {
                  newSet.add(current);
                  const nextParentIndex = current.lastIndexOf(separator);
                  if (nextParentIndex === -1 || current === relevantRoot) break;
                  current = current.substring(0, nextParentIndex);
                }
              }
              newSet.add(relevantRoot);
            }
            return { expandedFolders: newSet };
          });
        }

        if (isNewRoot) {
          if (path && !pinnedFolders.includes(path)) {
            // handleActiveTreeSectionChange('current');
            // Note: Update activeTreeSection via UI if needed, but here we just ensure state
          }
          setLibrary({ isTreeLoading: true });
          handleSettingsChange({ ...appSettings, lastRootPath: path } as any);
          try {
            const treeData = await invoke(Invokes.GetFolderTree, {
              path,
              expandedFolders: [path],
              showImageCounts: appSettings?.enableFolderImageCounts ?? false,
            });
            setLibrary({ folderTree: treeData });
          } catch (err) {
            console.error('Failed to load folder tree:', err);
            setError(`Failed to load folder tree: ${err}. Some sub-folders might be inaccessible.`);
          } finally {
            setLibrary({ isTreeLoading: false });
          }
        }

        setLibrary({ imageList: [], multiSelectedPaths: [], libraryActivePath: null });
        if (useEditorStore.getState().selectedImage) {
          debouncedSave.flush();
          debouncedSetHistory.cancel();
          setEditor({ selectedImage: null, finalPreviewUrl: null, uncroppedAdjustedPreviewUrl: null, histogram: null });
          setEditor({ adjustments: INITIAL_ADJUSTMENTS });
          resetHistory(INITIAL_ADJUSTMENTS);
        }

        const command =
          libraryViewMode === LibraryViewMode.Recursive ? Invokes.ListImagesRecursive : Invokes.ListImagesInDir;

        let files: ImageFile[];
        if (preloadedImages) {
          files = preloadedImages;
        } else {
          files = await invoke(command, { path });
        }

        const initialRatings: Record<string, number> = {};
        files.forEach((f) => {
          if (f.rating !== undefined) {
            initialRatings[f.path] = f.rating;
          }
        });
        setLibrary({ imageRatings: initialRatings });

        const exifSortKeys = ['date_taken', 'iso', 'shutter_speed', 'aperture', 'focal_length'];
        const isExifSortActive = exifSortKeys.includes(sortCriteria.key);
        const shouldReadExif = appSettings?.enableExifReading ?? false;

        if (shouldReadExif && files.length > 0) {
          const paths = files.map((f: ImageFile) => f.path);

          if (isExifSortActive) {
            const exifDataMap: Record<string, any> = await invoke(Invokes.ReadExifForPaths, { paths });
            const finalImageList = files.map((image) => ({
              ...image,
              exif: exifDataMap[image.path] || image.exif || null,
            }));
            setLibrary({ imageList: finalImageList });
          } else {
            setLibrary({ imageList: files });
            invoke(Invokes.ReadExifForPaths, { paths })
              .then((exifDataMap: any) => {
                setLibrary((state) => ({
                  imageList: state.imageList.map((image) => ({
                    ...image,
                    exif: exifDataMap[image.path] || image.exif || null,
                  })),
                }));
              })
              .catch((err) => {
                console.error('Failed to read EXIF data in background:', err);
              });
          }
        } else {
          setLibrary({ imageList: files });
        }

        invoke(Invokes.StartBackgroundIndexing, { folderPath: path }).catch((err) => {
          console.error('Failed to start background indexing:', err);
        });
      } catch (err) {
        console.error('Failed to load folder contents:', err);
        setError('Failed to load images from the selected folder.');
        setLibrary({ isTreeLoading: false });
      } finally {
        setLibrary({ isViewLoading: false });
      }
    },
    [
      appSettings,
      handleSettingsChange,
      rootPath,
      sortCriteria.key,
      pinnedFolders,
      libraryViewMode,
      debouncedSave,
      debouncedSetHistory,
      resetHistory,
      setUI,
      clearThumbnailQueue,
      setLibrary,
      setEditor,
      setProcess,
      refs,
      setError,
    ],
  );

  const handleOpenFolder = async () => {
    try {
      if (isAndroid) {
        const libraryRoot = await invoke<string>(Invokes.GetOrCreateInternalLibraryRoot);
        setLibrary({ rootPath: libraryRoot });
        await handleSelectSubfolder(libraryRoot, true);
        return;
      }

      const selected = await open({ directory: true, multiple: false, defaultPath: await homeDir() });
      if (typeof selected === 'string') {
        setLibrary({ rootPath: selected });
        await handleSelectSubfolder(selected, true);
      }
    } catch (err) {
      console.error(isAndroid ? 'Failed to open Android library root:' : 'Failed to open directory dialog:', err);
      setError(isAndroid ? 'Failed to open library.' : 'Failed to open folder selection dialog.');
    }
  };

  const handleContinueSession = () => {
    const restore = async () => {
      if (!appSettings?.lastRootPath) return;

      const root = appSettings.lastRootPath;
      const folderState = appSettings.lastFolderState;
      const pathToSelect = folderState?.currentFolderPath || root;

      setLibrary({ rootPath: root });

      if (folderState?.expandedFolders) {
        const newExpandedFolders = new Set<string>(folderState.expandedFolders);
        setLibrary({ expandedFolders: newExpandedFolders });
      } else {
        setLibrary({ expandedFolders: new Set([root]) });
      }

      setLibrary({ isTreeLoading: true });
      try {
        let treeData;
        if (preloadedDataRef.current.rootPath === root && preloadedDataRef.current.tree) {
          treeData = await preloadedDataRef.current.tree;
        } else {
          const expandedArr = folderState?.expandedFolders ? Array.from(new Set(folderState.expandedFolders)) : [root];
          treeData = await invoke(Invokes.GetFolderTree, {
            path: root,
            expandedFolders: expandedArr,
            showImageCounts: appSettings?.enableFolderImageCounts ?? false,
          });
        }
        setLibrary({ folderTree: treeData });
      } catch (err) {
        console.error('Failed to restore folder tree:', err);
      } finally {
        setLibrary({ isTreeLoading: false });
      }

      let preloadedImages: ImageFile[] | undefined = undefined;
      if (preloadedDataRef.current.currentPath === pathToSelect && preloadedDataRef.current.images) {
        try {
          preloadedImages = await preloadedDataRef.current.images;
        } catch (e) {
          console.error('Failed to retrieve preloaded images', e);
        }
      }

      await handleSelectSubfolder(pathToSelect, false, preloadedImages, false);
    };

    restore().catch((err) => {
      console.error('Failed to restore session, folder might be missing:', err);
      setError('Failed to restore session. The last used folder may have been moved or deleted.');
      if (appSettings) {
        handleSettingsChange({ ...appSettings, lastRootPath: null, lastFolderState: null });
      }
      handleGoHome();
      setLibrary({ isTreeLoading: false });
    });
  };

  return {
    handleGoHome,
    handleBackToLibrary,
    handleImageSelect,
    handleSelectSubfolder,
    handleOpenFolder,
    handleContinueSession,
  };
}
