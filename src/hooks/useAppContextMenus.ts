import { useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Aperture,
  Check,
  ClipboardPaste,
  Copy,
  CopyPlus,
  Edit,
  FileEdit,
  FileInput,
  Folder,
  FolderInput,
  FolderPlus,
  Images,
  LayoutTemplate,
  Redo,
  RefreshCw,
  RotateCcw,
  Star,
  SquaresUnite,
  Palette,
  Tag,
  Trash2,
  Undo,
  X,
  Pin,
  PinOff,
  Users,
  Gauge,
  Grip,
  Film,
} from 'lucide-react';
import { useContextMenu } from '../context/ContextMenuContext';
import { useEditorStore } from '../store/useEditorStore';
import { useLibraryStore } from '../store/useLibraryStore';
import { useProcessStore } from '../store/useProcessStore';
import { useUIStore } from '../store/useUIStore';
import { useSettingsStore } from '../store/useSettingsStore';
import { Invokes, Option, OPTION_SEPARATOR, Panel, ImageFile } from '../components/ui/AppProperties';
import {
  Color,
  COLOR_LABELS,
  COPYABLE_ADJUSTMENT_KEYS,
  INITIAL_ADJUSTMENTS,
  normalizeLoadedAdjustments,
} from '../utils/adjustments';
import TaggingSubMenu from '../context/TaggingSubMenu';

const RIGHT_PANEL_ORDER = [
  Panel.Metadata,
  Panel.Adjustments,
  Panel.Crop,
  Panel.Masks,
  Panel.Ai,
  Panel.Presets,
  Panel.Export,
];

export interface UseAppContextMenusProps {
  setError: (msg: string) => void;
  handleImageSelect: (path: string) => void;
  handleBackToLibrary: () => void;
  handleRenameFiles: (paths: string[]) => void;
  handleImportClick: (path: string) => void;
  handleLibraryRefresh: () => void;
  refreshAllFolderTrees: () => Promise<void>;
  refreshImageList: () => Promise<void>;
  executeDelete: (paths: string[], options: any) => Promise<void>;
  handleSetColorLabel: (color: string | null, paths?: string[]) => Promise<void>;
  handleRate: (rating: number, paths?: string[]) => void;
  handleCopyAdjustments: () => void;
  handlePasteAdjustments: (paths?: string[]) => void;
  handleAutoAdjustments: () => Promise<void>;
  handleTagsChanged: (paths: string[], tags: { tag: string; isUser: boolean }[]) => void;
  handleTogglePinFolder: (path: string) => Promise<void>;
  handleResetAdjustments: (paths?: string[]) => void;
  imageCacheRef: React.MutableRefObject<any>;
  copiedAdjustments: any;
  setCopiedAdjustments: React.Dispatch<React.SetStateAction<any>>;
}

export function useAppContextMenus(props: UseAppContextMenusProps) {
  const { showContextMenu } = useContextMenu();

  const setUI = useUIStore((state) => state.setUI);
  const setRightPanel = useUIStore((state) => state.setRightPanel);
  const setProcess = useProcessStore((state) => state.setProcess);
  const setLibrary = useLibraryStore((state) => state.setLibrary);
  const setEditor = useEditorStore((state) => state.setEditor);

  const appSettings = useSettingsStore((state) => state.appSettings);
  const selectedImage = useEditorStore((state) => state.selectedImage);
  const history = useEditorStore((state) => state.history);
  const historyIndex = useEditorStore((state) => state.historyIndex);
  const undo = useEditorStore((state) => state.undo);
  const redo = useEditorStore((state) => state.redo);
  const resetHistory = useEditorStore((state) => state.resetHistory);

  const imageList = useLibraryStore((state) => state.imageList);
  const multiSelectedPaths = useLibraryStore((state) => state.multiSelectedPaths);
  const libraryActivePath = useLibraryStore((state) => state.libraryActivePath);
  const rootPath = useLibraryStore((state) => state.rootPath);
  const currentFolderPath = useLibraryStore((state) => state.currentFolderPath);

  const copiedFilePaths = useProcessStore((state) => state.copiedFilePaths);

  const canUndo = historyIndex > 0;
  const canRedo = historyIndex < history.length - 1;

  const setLiveAdjustments = (adj: any) => setEditor({ adjustments: adj });

  const getCommonTags = useCallback(
    (paths: string[]): { tag: string; isUser: boolean }[] => {
      if (paths.length === 0) return [];
      const imageFiles = imageList.filter((img) => paths.includes(img.path));
      if (imageFiles.length === 0) return [];

      const allTagsSets = imageFiles.map((img) => {
        const tagsWithPrefix = (img.tags || []).filter((t: string) => !t.startsWith('color:'));
        return new Set(tagsWithPrefix);
      });

      if (allTagsSets.length === 0) return [];

      const commonTagsWithPrefix = allTagsSets.reduce((intersection, currentSet) => {
        return new Set([...intersection].filter((tag) => currentSet.has(tag)));
      });

      return Array.from(commonTagsWithPrefix)
        .map((tag: string) => ({
          tag: tag.startsWith('user:') ? tag.substring(5) : tag,
          isUser: tag.startsWith('user:'),
        }))
        .sort((a, b) => a.tag.localeCompare(b.tag));
    },
    [imageList],
  );

  const handleEditorContextMenu = (event: any) => {
    event.preventDefault();
    event.stopPropagation();
    if (!selectedImage) return;

    const commonTags = getCommonTags([selectedImage.path]);

    const options: Array<Option> = [
      {
        label: 'Export Image',
        icon: FileInput,
        onClick: () => {
          setRightPanel(Panel.Export, RIGHT_PANEL_ORDER);
        },
      },
      { type: OPTION_SEPARATOR },
      { label: 'Undo', icon: Undo, onClick: undo, disabled: !canUndo },
      { label: 'Redo', icon: Redo, onClick: redo, disabled: !canRedo },
      { type: OPTION_SEPARATOR },
      { label: 'Copy Adjustments', icon: Copy, onClick: props.handleCopyAdjustments },
      {
        label: 'Paste Adjustments',
        icon: ClipboardPaste,
        onClick: props.handlePasteAdjustments,
        disabled: props.copiedAdjustments === null,
      },
      {
        label: 'Productivity',
        icon: Gauge,
        submenu: [
          {
            label: 'Auto Adjust Image',
            icon: Aperture,
            onClick: props.handleAutoAdjustments,
            disabled: !selectedImage?.isReady,
          },
          {
            label: 'Denoise Image',
            icon: Grip,
            onClick: () => {
              setUI({
                denoiseModalState: {
                  isOpen: true,
                  isProcessing: false,
                  previewBase64: null,
                  error: null,
                  targetPaths: [selectedImage.path],
                  progressMessage: null,
                  isRaw: selectedImage?.isRaw || false,
                },
              });
            },
          },
          {
            label: 'Convert Negative',
            icon: Film,
            onClick: () => {
              if (selectedImage) {
                setUI({ negativeModalState: { isOpen: true, targetPaths: [selectedImage.path] } });
              }
            },
          },
          { disabled: true, icon: SquaresUnite, label: 'Stitch Panorama' },
          { disabled: true, icon: Images, label: 'Merge to HDR' },
          {
            icon: LayoutTemplate,
            label: 'Frame Image',
            onClick: () => {
              setUI({ collageModalState: { isOpen: true, sourceImages: [selectedImage] } });
            },
          },
          { label: 'Cull Image', icon: Users, disabled: true },
        ],
      },
      { type: OPTION_SEPARATOR },
      {
        label: 'Rating',
        icon: Star,
        submenu: [0, 1, 2, 3, 4, 5].map((rating: number) => ({
          label: rating === 0 ? 'No Rating' : `${rating} Star${rating !== 1 ? 's' : ''}`,
          onClick: () => props.handleRate(rating),
        })),
      },
      {
        label: 'Color Label',
        icon: Palette,
        submenu: [
          { label: 'No Label', onClick: () => props.handleSetColorLabel(null) },
          ...COLOR_LABELS.map((label: Color) => ({
            label: label.name.charAt(0).toUpperCase() + label.name.slice(1),
            color: label.color,
            onClick: () => props.handleSetColorLabel(label.name),
          })),
        ],
      },
      {
        label: 'Tagging',
        icon: Tag,
        submenu: [
          {
            customComponent: TaggingSubMenu,
            customProps: {
              paths: [selectedImage.path],
              initialTags: commonTags,
              onTagsChanged: props.handleTagsChanged,
              appSettings,
            },
          },
        ],
      },
      { type: OPTION_SEPARATOR },
      {
        label: 'Reset Adjustments',
        icon: RotateCcw,
        submenu: [
          { label: 'Cancel', icon: X, onClick: () => {} },
          {
            label: 'Confirm Reset',
            icon: Check,
            isDestructive: true,
            onClick: () => {
              const originalAspectRatio =
                selectedImage.width && selectedImage.height ? selectedImage.width / selectedImage.height : null;
              resetHistory({
                ...INITIAL_ADJUSTMENTS,
                aspectRatio: originalAspectRatio,
                aiPatches: [],
              });
            },
          },
        ],
      },
    ];
    showContextMenu(event.clientX, event.clientY, options);
  };

  const handleThumbnailContextMenu = (event: any, path: string) => {
    event.preventDefault();
    event.stopPropagation();

    const isTargetInSelection = multiSelectedPaths.includes(path);
    let finalSelection;

    if (!isTargetInSelection) {
      finalSelection = [path];
      setLibrary({ multiSelectedPaths: [path] });
      if (!selectedImage) {
        setLibrary({ libraryActivePath: path });
      }
    } else {
      finalSelection = multiSelectedPaths;
    }

    const commonTags = getCommonTags(finalSelection);

    const selectionCount = finalSelection.length;
    const isSingleSelection = selectionCount === 1;
    const isEditingThisImage = selectedImage?.path === path;
    const deleteLabel = isSingleSelection ? 'Delete Image' : `Delete ${selectionCount} Images`;
    const exportLabel = isSingleSelection ? 'Export Image' : `Export ${selectionCount} Images`;

    const selectionHasVirtualCopies =
      isSingleSelection &&
      !finalSelection[0].includes('?vc=') &&
      imageList.some((image) => image.path.startsWith(`${finalSelection[0]}?vc=`));

    const hasAssociatedFiles = finalSelection.some((selectedPath) => {
      const lastDotIndex = selectedPath.lastIndexOf('.');
      if (lastDotIndex === -1) return false;
      const basePath = selectedPath.substring(0, lastDotIndex);
      return imageList.some((image) => image.path.startsWith(basePath + '.') && image.path !== selectedPath);
    });

    let deleteSubmenu;
    if (selectionHasVirtualCopies) {
      deleteSubmenu = [
        { label: 'Cancel', icon: X, onClick: () => {} },
        {
          label: 'Confirm Delete + Virtual Copies',
          icon: Check,
          isDestructive: true,
          onClick: () => props.executeDelete(finalSelection, { includeAssociated: false }),
        },
      ];
    } else if (hasAssociatedFiles) {
      deleteSubmenu = [
        { label: 'Cancel', icon: X, onClick: () => {} },
        {
          label: 'Delete Selected Only',
          icon: Check,
          isDestructive: true,
          onClick: () => props.executeDelete(finalSelection, { includeAssociated: false }),
        },
        {
          label: 'Delete + Associated',
          icon: Check,
          isDestructive: true,
          onClick: () => props.executeDelete(finalSelection, { includeAssociated: true }),
        },
      ];
    } else {
      deleteSubmenu = [
        { label: 'Cancel', icon: X, onClick: () => {} },
        {
          label: 'Confirm Delete',
          icon: Check,
          isDestructive: true,
          onClick: () => props.executeDelete(finalSelection, { includeAssociated: false }),
        },
      ];
    }

    const pasteLabel = isSingleSelection ? 'Paste Adjustments' : `Paste Adjustments to ${selectionCount} Images`;
    const resetLabel = isSingleSelection ? 'Reset Adjustments' : `Reset Adjustments on ${selectionCount} Images`;
    const copyLabel = isSingleSelection ? 'Copy Image' : `Copy ${selectionCount} Images`;
    const autoAdjustLabel = isSingleSelection ? 'Auto Adjust Image' : `Auto Adjust Images`;
    const renameLabel = isSingleSelection ? 'Rename Image' : `Rename ${selectionCount} Images`;
    const cullLabel = isSingleSelection ? 'Cull Image' : `Cull Images`;
    const collageLabel = isSingleSelection ? 'Frame Image' : 'Create Collage';
    const stitchLabel = 'Stitch Panorama';
    const conversionLabel = isSingleSelection ? 'Convert Negative' : 'Convert Negatives';
    const denoiseLabel = isSingleSelection ? 'Denoise Image' : 'Denoise Images';
    const mergeLabel = `Merge to HDR`;

    const handleCreateVirtualCopy = async (sourcePath: string) => {
      try {
        await invoke(Invokes.CreateVirtualCopy, { sourceVirtualPath: sourcePath });
        await props.refreshImageList();
      } catch (err) {
        console.error('Failed to create virtual copy:', err);
        props.setError(`Failed to create virtual copy: ${err}`);
      }
    };

    const handleApplyAutoAdjustmentsToSelection = () => {
      if (finalSelection.length === 0) return;
      finalSelection.forEach((p) => props.imageCacheRef.current.delete(p));

      invoke(Invokes.ApplyAutoAdjustmentsToPaths, { paths: finalSelection })
        .then(async () => {
          if (selectedImage && finalSelection.includes(selectedImage.path)) {
            const metadata: any = await invoke(Invokes.LoadMetadata, { path: selectedImage.path });
            if (metadata.adjustments && !metadata.adjustments.is_null) {
              const normalized = normalizeLoadedAdjustments(metadata.adjustments);
              setLiveAdjustments(normalized);
              resetHistory(normalized);
            }
          }
          if (libraryActivePath && finalSelection.includes(libraryActivePath)) {
            const metadata: any = await invoke(Invokes.LoadMetadata, { path: libraryActivePath });
            if (metadata.adjustments && !metadata.adjustments.is_null) {
              const normalized = normalizeLoadedAdjustments(metadata.adjustments);
              setLibrary({ libraryActiveAdjustments: normalized });
            }
          }
        })
        .catch((err) => {
          console.error('Failed to apply auto adjustments to paths:', err);
          props.setError(`Failed to apply auto adjustments: ${err}`);
        });
    };

    const onExportClick = () => {
      if (selectedImage) {
        if (selectedImage.path !== path) {
          props.handleImageSelect(path);
        }
        setLibrary({ multiSelectedPaths: finalSelection });
        setRightPanel(Panel.Export, RIGHT_PANEL_ORDER);
      } else {
        setLibrary({ multiSelectedPaths: finalSelection });
        setUI({ isLibraryExportPanelVisible: true });
      }
    };

    const options = [
      ...(!isEditingThisImage
        ? [
            {
              disabled: !isSingleSelection,
              icon: Edit,
              label: 'Edit Image',
              onClick: () => props.handleImageSelect(finalSelection[0]),
            },
            { icon: FileInput, label: exportLabel, onClick: onExportClick },
            { type: OPTION_SEPARATOR },
          ]
        : [{ icon: FileInput, label: exportLabel, onClick: onExportClick }, { type: OPTION_SEPARATOR }]),
      {
        disabled: !isSingleSelection,
        icon: Copy,
        label: 'Copy Adjustments',
        onClick: async () => {
          try {
            const metadata: any = await invoke(Invokes.LoadMetadata, { path: finalSelection[0] });
            const sourceAdjustments =
              metadata.adjustments && !metadata.adjustments.is_null
                ? { ...INITIAL_ADJUSTMENTS, ...metadata.adjustments }
                : INITIAL_ADJUSTMENTS;
            const adjustmentsToCopy: any = {};
            for (const key of COPYABLE_ADJUSTMENT_KEYS) {
              if (Object.prototype.hasOwnProperty.call(sourceAdjustments, key)) {
                adjustmentsToCopy[key] = (sourceAdjustments as any)[key];
              }
            }
            props.setCopiedAdjustments(adjustmentsToCopy);
            setProcess({ isCopied: true });
          } catch (err) {
            console.error('Failed to load metadata for copy:', err);
            props.setError(`Failed to copy adjustments: ${err}`);
          }
        },
      },
      {
        disabled: props.copiedAdjustments === null,
        icon: ClipboardPaste,
        label: pasteLabel,
        onClick: () => props.handlePasteAdjustments(finalSelection),
      },
      {
        label: 'Productivity',
        icon: Gauge,
        submenu: [
          { label: autoAdjustLabel, icon: Aperture, onClick: handleApplyAutoAdjustmentsToSelection },
          {
            label: denoiseLabel,
            icon: Grip,
            disabled: finalSelection.length === 0,
            onClick: () => {
              setUI({
                denoiseModalState: {
                  isOpen: true,
                  isProcessing: false,
                  previewBase64: null,
                  error: null,
                  targetPaths: finalSelection,
                  progressMessage: null,
                  isRaw: selectedImage?.isRaw || false,
                },
              });
            },
          },
          {
            label: conversionLabel,
            icon: Film,
            disabled: selectionCount === 0,
            onClick: () => {
              setUI({ negativeModalState: { isOpen: true, targetPaths: finalSelection } });
            },
          },
          {
            disabled: selectionCount < 2 || selectionCount > 30,
            icon: SquaresUnite,
            label: stitchLabel,
            onClick: () => {
              setUI({
                panoramaModalState: {
                  error: null,
                  finalImageBase64: null,
                  isOpen: true,
                  isProcessing: false,
                  progressMessage: null,
                  stitchingSourcePaths: finalSelection,
                },
              });
            },
          },
          {
            disabled: selectionCount < 2 || selectionCount > 9,
            icon: Images,
            label: mergeLabel,
            onClick: () => {
              setUI({
                hdrModalState: {
                  error: null,
                  finalImageBase64: null,
                  isOpen: true,
                  isProcessing: false,
                  progressMessage: null,
                  stitchingSourcePaths: finalSelection,
                },
              });
            },
          },
          {
            icon: LayoutTemplate,
            label: collageLabel,
            onClick: () => {
              const imagesForCollage = imageList.filter((img) => finalSelection.includes(img.path));
              setUI({ collageModalState: { isOpen: true, sourceImages: imagesForCollage } });
            },
            disabled: selectionCount === 0 || selectionCount > 9,
          },
          {
            label: cullLabel,
            icon: Users,
            onClick: () =>
              setUI({
                cullingModalState: {
                  isOpen: true,
                  progress: null,
                  suggestions: null,
                  error: null,
                  pathsToCull: finalSelection,
                },
              }),
            disabled: selectionCount < 2,
          },
        ],
      },
      { type: OPTION_SEPARATOR },
      {
        label: copyLabel,
        icon: Copy,
        onClick: () => {
          setProcess({ copiedFilePaths: finalSelection, isCopied: true });
        },
      },
      {
        icon: CopyPlus,
        label: 'Duplicate Image',
        disabled: !isSingleSelection,
        submenu: [
          {
            label: 'Physical Copy',
            icon: Copy,
            onClick: async () => {
              try {
                await invoke(Invokes.DuplicateFile, { path: finalSelection[0] });
                await props.refreshImageList();
              } catch (err) {
                console.error('Failed to duplicate file:', err);
                props.setError(`Failed to duplicate file: ${err}`);
              }
            },
          },
          {
            label: 'Virtual Copy',
            icon: CopyPlus,
            onClick: () => handleCreateVirtualCopy(finalSelection[0]),
          },
        ],
      },
      { icon: FileEdit, label: renameLabel, onClick: () => props.handleRenameFiles(finalSelection) },
      { type: OPTION_SEPARATOR },
      {
        icon: Star,
        label: 'Rating',
        submenu: [0, 1, 2, 3, 4, 5].map((rating: number) => ({
          label: rating === 0 ? 'No Rating' : `${rating} Star${rating !== 1 ? 's' : ''}`,
          onClick: () => props.handleRate(rating, finalSelection),
        })),
      },
      {
        label: 'Color Label',
        icon: Palette,
        submenu: [
          { label: 'No Label', onClick: () => props.handleSetColorLabel(null, finalSelection) },
          ...COLOR_LABELS.map((label: Color) => ({
            label: label.name.charAt(0).toUpperCase() + label.name.slice(1),
            color: label.color,
            onClick: () => props.handleSetColorLabel(label.name, finalSelection),
          })),
        ],
      },
      {
        label: 'Tagging',
        icon: Tag,
        submenu: [
          {
            customComponent: TaggingSubMenu,
            customProps: {
              paths: finalSelection,
              initialTags: commonTags,
              onTagsChanged: props.handleTagsChanged,
              appSettings,
            },
          },
        ],
      },
      { type: OPTION_SEPARATOR },
      {
        disabled: !isSingleSelection,
        icon: Folder,
        label: 'Show in File Explorer',
        onClick: () => {
          invoke(Invokes.ShowInFinder, { path: finalSelection[0] }).catch((err) =>
            props.setError(`Could not show file in explorer: ${err}`),
          );
        },
      },
      {
        label: resetLabel,
        icon: RotateCcw,
        submenu: [
          { label: 'Cancel', icon: X, onClick: () => {} },
          {
            label: 'Confirm Reset',
            icon: Check,
            isDestructive: true,
            onClick: () => props.handleResetAdjustments(finalSelection),
          },
        ],
      },
      {
        label: deleteLabel,
        icon: Trash2,
        isDestructive: true,
        submenu: deleteSubmenu,
      },
    ];
    showContextMenu(event.clientX, event.clientY, options);
  };

  const handleFolderTreeContextMenu = (event: any, path: string, isCurrentlyPinned?: boolean) => {
    event.preventDefault();
    event.stopPropagation();
    const targetPath = path || rootPath;
    if (!targetPath) return;

    const isRoot = targetPath === rootPath;
    const numCopied = copiedFilePaths.length;
    const copyPastedLabel = numCopied === 1 ? 'Copy image here' : `Copy ${numCopied} images here`;
    const movePastedLabel = numCopied === 1 ? 'Move image here' : `Move ${numCopied} images here`;

    const pinOption = isCurrentlyPinned
      ? { icon: PinOff, label: 'Unpin Folder', onClick: () => props.handleTogglePinFolder(targetPath) }
      : { icon: Pin, label: 'Pin Folder', onClick: () => props.handleTogglePinFolder(targetPath) };

    const options = [
      pinOption,
      { type: OPTION_SEPARATOR },
      {
        icon: FolderPlus,
        label: 'New Folder',
        onClick: () => {
          setUI({ folderActionTarget: targetPath, isCreateFolderModalOpen: true });
        },
      },
      {
        disabled: isRoot,
        icon: FileEdit,
        label: 'Rename Folder',
        onClick: () => {
          setUI({ folderActionTarget: targetPath, isRenameFolderModalOpen: true });
        },
      },
      { type: OPTION_SEPARATOR },
      {
        disabled: copiedFilePaths.length === 0,
        icon: ClipboardPaste,
        label: 'Paste',
        submenu: [
          {
            label: copyPastedLabel,
            onClick: async () => {
              try {
                await invoke(Invokes.CopyFiles, { sourcePaths: copiedFilePaths, destinationFolder: targetPath });
                if (targetPath === currentFolderPath) props.handleLibraryRefresh();
              } catch (err) {
                props.setError(`Failed to copy files: ${err}`);
              }
            },
          },
          {
            label: movePastedLabel,
            onClick: async () => {
              try {
                await invoke(Invokes.MoveFiles, { sourcePaths: copiedFilePaths, destinationFolder: targetPath });
                setProcess({ copiedFilePaths: [] });
                setLibrary({ multiSelectedPaths: [] });
                props.refreshAllFolderTrees();
                props.handleLibraryRefresh();
              } catch (err) {
                props.setError(`Failed to move files: ${err}`);
              }
            },
          },
        ],
      },
      { icon: FolderInput, label: 'Import Images', onClick: () => props.handleImportClick(targetPath) },
      { type: OPTION_SEPARATOR },
      {
        icon: Folder,
        label: 'Show in File Explorer',
        onClick: () =>
          invoke(Invokes.ShowInFinder, { path: targetPath }).catch((err) =>
            props.setError(`Could not show folder: ${err}`),
          ),
      },
      ...(path
        ? [
            {
              disabled: isRoot,
              icon: Trash2,
              isDestructive: true,
              label: 'Delete Folder',
              submenu: [
                { label: 'Cancel', icon: X, onClick: () => {} },
                {
                  label: 'Confirm',
                  icon: Check,
                  isDestructive: true,
                  onClick: async () => {
                    try {
                      await invoke(Invokes.DeleteFolder, { path: targetPath });
                      if (currentFolderPath?.startsWith(targetPath)) {
                        // This requires rootPath, which should be handled appropriately in app, but since it's an action on path:
                        // We leave the side effect to refreshAllFolderTrees
                      }
                      props.refreshAllFolderTrees();
                    } catch (err) {
                      props.setError(`Failed to delete folder: ${err}`);
                    }
                  },
                },
              ],
            },
          ]
        : []),
    ];
    showContextMenu(event.clientX, event.clientY, options);
  };

  const handleMainLibraryContextMenu = (event: any) => {
    event.preventDefault();
    event.stopPropagation();
    const numCopied = copiedFilePaths.length;
    const copyPastedLabel = numCopied === 1 ? 'Copy image here' : `Copy ${numCopied} images here`;
    const movePastedLabel = numCopied === 1 ? 'Move image here' : `Move ${numCopied} images here`;

    const options = [
      { label: 'Refresh Folder', icon: RefreshCw, onClick: props.handleLibraryRefresh },
      { type: OPTION_SEPARATOR },
      {
        label: 'Paste',
        icon: ClipboardPaste,
        disabled: copiedFilePaths.length === 0,
        submenu: [
          {
            label: copyPastedLabel,
            onClick: async () => {
              try {
                await invoke(Invokes.CopyFiles, { sourcePaths: copiedFilePaths, destinationFolder: currentFolderPath });
                props.handleLibraryRefresh();
              } catch (err) {
                props.setError(`Failed to copy files: ${err}`);
              }
            },
          },
          {
            label: movePastedLabel,
            onClick: async () => {
              try {
                await invoke(Invokes.MoveFiles, { sourcePaths: copiedFilePaths, destinationFolder: currentFolderPath });
                setProcess({ copiedFilePaths: [] });
                setLibrary({ multiSelectedPaths: [] });
                props.refreshAllFolderTrees();
                props.handleLibraryRefresh();
              } catch (err) {
                props.setError(`Failed to move files: ${err}`);
              }
            },
          },
        ],
      },
      {
        icon: FolderInput,
        label: 'Import Images',
        onClick: () => props.handleImportClick(currentFolderPath as string),
        disabled: !currentFolderPath,
      },
    ];
    showContextMenu(event.clientX, event.clientY, options);
  };

  return {
    handleEditorContextMenu,
    handleThumbnailContextMenu,
    handleFolderTreeContextMenu,
    handleMainLibraryContextMenu,
  };
}
