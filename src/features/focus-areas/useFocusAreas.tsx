import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'react-toastify';
import type { SelectedImage } from '../../components/ui/AppProperties';
import { FOCUS_AREAS_INVOKE } from './constants';
import type { FocusAreasController } from './contracts';
import type { FocusRegion } from './types';

interface UseFocusAreasResult extends FocusAreasController {
  focusAreasError: string | null;
}

export function useFocusAreas(selectedImage: SelectedImage | null): UseFocusAreasResult {
  const [showFocusAreas, setShowFocusAreas] = useState(false);
  const [focusRegions, setFocusRegions] = useState<FocusRegion[]>([]);
  const [focusAreasError, setFocusAreasError] = useState<string | null>(null);

  useEffect(() => {
    if (!selectedImage?.path || !showFocusAreas) {
      setFocusRegions([]);
      return;
    }

    invoke<FocusRegion[]>(FOCUS_AREAS_INVOKE, {
      params: {
        path: selectedImage.path,
        imageWidth: selectedImage.width,
        imageHeight: selectedImage.height,
      },
    })
      .then((regions) => {
        setFocusRegions(regions);
        setFocusAreasError(null);
      })
      .catch((err) => {
        setFocusRegions([]);
        setFocusAreasError(err);
        toast.info(`对焦区域显示不可用\n${err}\n\n您可以提交样本文件帮助我们添加支持`, {
          autoClose: 5000,
        });
      });
  }, [selectedImage?.path, selectedImage?.width, selectedImage?.height, showFocusAreas]);

  const toggleFocusAreas = useCallback(() => {
    setShowFocusAreas((prev) => !prev);
  }, []);

  return {
    focusAreasError,
    focusRegions,
    showFocusAreas,
    toggleFocusAreas,
  };
}
