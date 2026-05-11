import { useCallback, useEffect, useRef, useState } from 'react';
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
  const lastNoticeKeyRef = useRef<string | null>(null);

  useEffect(() => {
    if (!selectedImage?.path || !selectedImage.width || !selectedImage.height || !showFocusAreas) {
      setFocusRegions([]);
      setFocusAreasError(null);
      return;
    }

    let isActive = true;
    const requestPath = selectedImage.path;
    const noticePrefix = `${requestPath}:${selectedImage.width}x${selectedImage.height}`;

    invoke<FocusRegion[]>(FOCUS_AREAS_INVOKE, {
      params: {
        path: requestPath,
        imageWidth: selectedImage.width,
        imageHeight: selectedImage.height,
      },
    })
      .then((regions) => {
        if (!isActive) {
          return;
        }

        setFocusRegions(regions);

        if (regions.length === 0) {
          const emptyMessage = '未找到可用的对焦区域元数据，当前相机或 RAW 格式可能暂不支持';
          setFocusAreasError(emptyMessage);

          const noticeKey = `${noticePrefix}:empty`;
          if (lastNoticeKeyRef.current !== noticeKey) {
            lastNoticeKeyRef.current = noticeKey;
            toast.info(`${emptyMessage}\n\n您可以提交样本文件帮助我们添加支持`, {
              autoClose: 5000,
            });
          }
        } else {
          setFocusAreasError(null);
          lastNoticeKeyRef.current = null;
        }
      })
      .catch((err) => {
        if (!isActive) {
          return;
        }

        const message = String(err);
        setFocusRegions([]);
        setFocusAreasError(message);

        const noticeKey = `${noticePrefix}:error:${message}`;
        if (lastNoticeKeyRef.current !== noticeKey) {
          lastNoticeKeyRef.current = noticeKey;
          toast.info(`对焦区域显示不可用\n${message}\n\n您可以提交样本文件帮助我们添加支持`, {
            autoClose: 5000,
          });
        }
      });

    return () => {
      isActive = false;
    };
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
