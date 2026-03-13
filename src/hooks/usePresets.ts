import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import debounce from 'lodash.debounce';
import { Adjustments, COPYABLE_ADJUSTMENT_KEYS } from '../utils/adjustments';
import { Folder, Invokes, Preset } from '../components/ui/AppProperties';

export enum PresetListType {
  Folder = 'folder',
  Preset = 'preset',
}

export interface UserPreset {
  folder?: Folder;
  id?: string | undefined;
  name?: string | undefined;
  preset?: Preset;
}

function arrayMove<T>(array: T[], from: number, to: number) {
  const newArray = array.slice();
  const [item] = newArray.splice(from, 1);
  newArray.splice(to, 0, item);
  return newArray;
}

export function usePresets(currentAdjustments: Adjustments) {
  const [presets, setPresets] = useState<Array<UserPreset>>([]);
  const [isLoading, setIsLoading] = useState(true);

  const loadPresets = useCallback(async () => {
    setIsLoading(true);
    try {
      const loadedPresets: Array<UserPreset> = await invoke(Invokes.LoadPresets);
      console.log(loadedPresets);
      setPresets(loadedPresets);
    } catch (error) {
      console.error('Failed to load presets:', error);
      setPresets([]);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const savePresetsToBackend = useCallback(
    debounce((presetsToSave: Array<UserPreset>) => {
      console.log(presetsToSave);
      invoke(Invokes.SavePresets, { presets: presetsToSave }).catch((err) =>
        console.error('Failed to save presets:', err),
      );
    }, 500),
    [],
  );

  useEffect(() => {
    loadPresets();
  }, [loadPresets]);

  const addPreset = (name: string, folderId = null) => {
    const presetAdjustments: Record<string, unknown> = {};
    for (const key of COPYABLE_ADJUSTMENT_KEYS) {
      if (Object.prototype.hasOwnProperty.call(currentAdjustments, key)) {
        presetAdjustments[key] = currentAdjustments[key];
      }
    }

    const newPresetData: Preset = {
      adjustments: presetAdjustments,
      id: crypto.randomUUID(),
      name,
    };

    let updatedPresets: Array<UserPreset>;
    if (folderId) {
      updatedPresets = presets.map((item: UserPreset) => {
        if (item.folder && item.folder.id === folderId) {
          return {
            folder: {
              ...item.folder,
              children: [...item.folder.children, newPresetData],
            },
          };
        }
        return item;
      }) as UserPreset[];
    } else {
      updatedPresets = [...presets, { preset: newPresetData }];
    }

    setPresets(updatedPresets);
    savePresetsToBackend(updatedPresets);
    return newPresetData;
  };

  const addFolder = (name: string) => {
    const newFolder = {
      folder: {
        id: crypto.randomUUID(),
        name,
        children: [],
      },
    };

    setPresets((currentPresets: Array<UserPreset>) => {
      console.log(currentPresets);
      const updatedPresets = [...currentPresets];
      const firstPresetIndex = updatedPresets.findIndex((p: UserPreset) => p.preset);

      if (firstPresetIndex === -1) {
        updatedPresets.push(newFolder);
      } else {
        updatedPresets.splice(firstPresetIndex, 0, newFolder);
      }

      savePresetsToBackend(updatedPresets);
      return updatedPresets;
    });
  };

  const deleteItem = (id: string) => {
    let updatedPresets = presets.filter((item: UserPreset) => item.preset?.id !== id && item.folder?.id !== id);
    updatedPresets = updatedPresets.map((item: UserPreset) => {
      if (item.folder) {
        return {
          folder: {
            ...item.folder,
            children: item.folder.children.filter((child) => (child as Preset).id !== id),
          },
        };
      }
      return item;
    }) as UserPreset[];
    setPresets(updatedPresets);
    savePresetsToBackend(updatedPresets);
  };

  const renameItem = (id: string | null, newName: string) => {
    const updatedPresets = presets.map((item: UserPreset) => {
      if (item.preset?.id === id) {
        return { preset: { ...item.preset, name: newName } };
      }
      if (item.folder?.id === id && item.folder) {
        return { folder: { ...item.folder!, name: newName } };
      }
      if (item.folder) {
        return {
          folder: {
            ...item.folder,
            children: item.folder.children.map((child) => {
              const p = child as Preset;
              return p.id === id ? { ...p, name: newName } : child;
            }),
          },
        };
      }
      return item;
    });
    setPresets(updatedPresets);
    savePresetsToBackend(updatedPresets);
  };

  const updatePreset = (id: string | null) => {
    const presetAdjustments: Record<string, unknown> = {};
    for (const key of COPYABLE_ADJUSTMENT_KEYS) {
      if (Object.prototype.hasOwnProperty.call(currentAdjustments, key)) {
        presetAdjustments[key] = currentAdjustments[key];
      }
    }

    let updatedPreset = null;
    const updatedPresets = presets.map((item: UserPreset) => {
      if (item.preset?.id === id) {
        updatedPreset = { ...item.preset, adjustments: presetAdjustments };
        return { preset: updatedPreset };
      }
      if (item.folder) {
        let found = false;
        const newChildren = item.folder.children.map((child) => {
          const p = child as Preset;
          if (p.id === id) {
            found = true;
            updatedPreset = { ...p, adjustments: presetAdjustments };
            return updatedPreset;
          }
          return child;
        });
        if (found) {
          return { folder: { ...item.folder, children: newChildren } };
        }
      }
      return item;
    });
    setPresets(updatedPresets);
    savePresetsToBackend(updatedPresets);
    return updatedPreset;
  };

  const duplicatePreset = useCallback(
    (presetId: string | null) => {
      let presetToDuplicate: Preset | null = null;
      let sourceFolderId: string | undefined | null = null;

      for (const item of presets) {
        if (item.preset?.id === presetId) {
          presetToDuplicate = item.preset;
          break;
        }
        if (item.folder) {
          const found = item.folder.children.find((p) => (p as Preset).id === presetId);
          if (found) {
            presetToDuplicate = found as Preset;
            sourceFolderId = item.folder.id;
            break;
          }
        }
      }

      if (!presetToDuplicate) {
        console.error('Preset to duplicate not found');
        return null;
      }

      const newPreset = {
        adjustments: JSON.parse(JSON.stringify(presetToDuplicate.adjustments)),
        id: crypto.randomUUID(),
        name: `${presetToDuplicate.name} Copy`,
      };

      let updatedPresets;
      if (sourceFolderId) {
        updatedPresets = presets.map((item: UserPreset) => {
          if (item.folder?.id === sourceFolderId && item.folder) {
            const originalIndex = item.folder!.children.findIndex((p) => (p as Preset).id === presetId);
            const newChildren = [...item.folder!.children];
            newChildren.splice(originalIndex + 1, 0, newPreset);
            return { folder: { ...item.folder!, children: newChildren } };
          }
          return item;
        });
      } else {
        const originalIndex = presets.findIndex((item: UserPreset) => item.preset?.id === presetId);
        updatedPresets = [...presets];
        updatedPresets.splice(originalIndex + 1, 0, { preset: newPreset });
      }

      setPresets(updatedPresets);
      savePresetsToBackend(updatedPresets);
      return newPreset;
    },
    [presets, savePresetsToBackend],
  );

  const movePreset = useCallback(
    (presetId: string, targetFolderId: string | null, overId: string | null = null) => {
      let presetToMove: Preset | null = null;
      let sourceFolderId: string | undefined | null = null;

      for (const item of presets) {
        if (item.preset?.id === presetId) {
          presetToMove = item.preset;
          break;
        }
        if (item.folder) {
          const found = item.folder.children.find((p) => (p as Preset).id === presetId);
          if (found) {
            presetToMove = found as Preset;
            sourceFolderId = item.folder.id;
            break;
          }
        }
      }

      if (!presetToMove) {
        return;
      }

      let updatedPresets = [...presets];

      if (sourceFolderId) {
        updatedPresets = updatedPresets.map((item: UserPreset) => {
          if (item.folder?.id === sourceFolderId && item.folder) {
            return {
              folder: {
                ...item.folder!,
                children: item.folder!.children.filter((p) => (p as Preset).id !== presetId),
              },
            };
          }
          return item;
        });
      } else {
        updatedPresets = updatedPresets.filter((item: UserPreset) => item.preset?.id !== presetId);
      }

      if (targetFolderId) {
        updatedPresets = updatedPresets.map((item: UserPreset) => {
          if (item.folder?.id === targetFolderId && item.folder) {
            const newChildren = [...item.folder!.children];
            if (overId) {
              const overIndex = newChildren.findIndex((p) => (p as Preset).id === overId);
              if (overIndex !== -1) {
                newChildren.splice(overIndex, 0, presetToMove as Preset);
              } else {
                newChildren.push(presetToMove as Preset);
              }
            } else {
              newChildren.push(presetToMove as Preset);
            }
            return { folder: { ...item.folder!, children: newChildren } };
          }
          return item;
        });
      } else {
        if (overId) {
          const overIndex = updatedPresets.findIndex(
            (item) => item.preset?.id === overId || item.folder?.id === overId,
          );
          if (overIndex !== -1) {
            updatedPresets.splice(overIndex, 0, { preset: presetToMove as Preset });
          } else {
            updatedPresets.push({ preset: presetToMove as Preset });
          }
        } else {
          updatedPresets.push({ preset: presetToMove as Preset });
        }
      }

      setPresets(updatedPresets);
      savePresetsToBackend(updatedPresets);
    },
    [presets, savePresetsToBackend],
  );

  const reorderItems = useCallback(
    (activeId: string, overId: string) => {
      setPresets((currentPresets: Array<UserPreset>) => {
        const getIndex = (arr: Array<UserPreset>, id: string) =>
          arr.findIndex((item: UserPreset) => item.preset?.id === id || item.folder?.id === id || item?.id === id);

        const activeRootIndex = getIndex(currentPresets, activeId);
        const overRootIndex = getIndex(currentPresets, overId);

        if (activeRootIndex !== -1 && overRootIndex !== -1) {
          const newPresets: Array<UserPreset> = arrayMove(currentPresets, activeRootIndex, overRootIndex);
          savePresetsToBackend(newPresets);
          return newPresets;
        }

        for (const item of currentPresets) {
          if (item.folder) {
            const childrenAsUserPresets = item.folder.children as unknown as Array<UserPreset>;
            const activeChildIndex = getIndex(childrenAsUserPresets, activeId);
            const overChildIndex = getIndex(childrenAsUserPresets, overId);

            if (activeChildIndex !== -1 && overChildIndex !== -1) {
              const newPresets = currentPresets.map((p: UserPreset) => {
                if (p.folder?.id === item.folder?.id && p.folder) {
                  return {
                    folder: {
                      ...p.folder!,
                      children: arrayMove(p.folder!.children, activeChildIndex, overChildIndex),
                    },
                  };
                }
                return p;
              });
              savePresetsToBackend(newPresets as Array<UserPreset>);
              return newPresets as Array<UserPreset>;
            }
          }
        }

        return currentPresets;
      });
    },
    [savePresetsToBackend],
  );

  const sortAllPresetsAlphabetically = useCallback(() => {
    setPresets((currentPresets) => {
      const newPresets: Array<UserPreset> = JSON.parse(JSON.stringify(currentPresets));
      const sortOptions: Intl.CollatorOptions = {
        numeric: true,
        sensitivity: 'base' as Intl.CollatorOptions['sensitivity'],
      };

      newPresets.forEach((item: UserPreset) => {
        if (item.folder && item.folder.children) {
          item.folder.children.sort((a, b) =>
            ((a as Preset).name ?? (a as Folder).name ?? '').localeCompare(
              (b as Preset).name ?? (b as Folder).name ?? '',
              undefined,
              sortOptions,
            ),
          );
        }
      });

      const folders = newPresets.filter((item: UserPreset) => item.folder);
      const rootPresets = newPresets.filter((item: UserPreset) => item.preset);

      folders.sort((a: UserPreset, b: UserPreset) =>
        (a.folder?.name ?? '').localeCompare(b.folder?.name ?? '', undefined, sortOptions),
      );
      rootPresets.sort((a: UserPreset, b: UserPreset) =>
        (a.preset?.name ?? '').localeCompare(b.preset?.name ?? '', undefined, sortOptions),
      );

      const sortedPresets = [...folders, ...rootPresets];

      savePresetsToBackend(sortedPresets);
      return sortedPresets;
    });
  }, [savePresetsToBackend]);

  const importPresetsFromFile = useCallback(
    async (filePath: string) => {
      setIsLoading(true);
      try {
        const updatedPresetList: Array<UserPreset> = await invoke(Invokes.HandleImportPresetsFromFile, { filePath });
        setPresets(updatedPresetList);
      } catch (error) {
        console.error('Failed to import presets from file:', error);
        throw error;
      } finally {
        setIsLoading(false);
      }
    },
    [setPresets],
  );

  const importLegacyPresetsFromFile = useCallback(
    async (filePath: string) => {
      setIsLoading(true);
      try {
        const updatedPresetList: Array<UserPreset> = await invoke(Invokes.HandleImportLegacyPresetsFromFile, {
          filePath,
        });
        setPresets(updatedPresetList);
      } catch (error) {
        console.error('Failed to import legacy presets from file:', error);
        throw error;
      } finally {
        setIsLoading(false);
      }
    },
    [setPresets],
  );

  const exportPresetsToFile = useCallback(async (presetsToExport: Array<UserPreset>, filePath: string) => {
    try {
      await invoke(Invokes.HandleExportPresetsToFile, { presetsToExport, filePath });
    } catch (error) {
      console.error('Failed to export presets to file:', error);
      throw error;
    }
  }, []);

  return {
    addFolder,
    addPreset,
    deleteItem,
    duplicatePreset,
    exportPresetsToFile,
    importPresetsFromFile,
    importLegacyPresetsFromFile,
    isLoading,
    movePreset,
    presets,
    refreshPresets: loadPresets,
    renameItem,
    reorderItems,
    sortAllPresetsAlphabetically,
    updatePreset,
  };
}
