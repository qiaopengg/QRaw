import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
import { Upload, X, RefreshCw } from 'lucide-react';

interface ReferencePreviewTileProps {
  badge: string;
  path: string;
  onRemove?: () => void;
  onReplace?: () => void;
  isMain?: boolean;
}

function basename(path: string) {
  return path.split(/[\\/]/).pop() || path;
}

function ReferencePreviewTile({ badge, path, onRemove, onReplace, isMain = false }: ReferencePreviewTileProps) {
  const [previewFailed, setPreviewFailed] = useState(false);
  const [thumbnailData, setThumbnailData] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [errorMessage, setErrorMessage] = useState<string>('');

  // 生成缩略图
  useEffect(() => {
    let cancelled = false;

    async function loadThumbnail() {
      try {
        setIsLoading(true);
        setErrorMessage('');
        const result = await invoke<Record<string, string>>('generate_thumbnails', {
          paths: [path],
        });

        if (!cancelled) {
          const thumbnailValue = result[path];

          if (thumbnailValue && thumbnailValue.startsWith('ERROR:')) {
            setPreviewFailed(true);
            setErrorMessage(thumbnailValue.replace('ERROR:', ''));
          } else if (thumbnailValue && thumbnailValue.startsWith('data:image/')) {
            setThumbnailData(thumbnailValue);
            setPreviewFailed(false);
          } else if (thumbnailValue) {
            setThumbnailData(thumbnailValue);
            setPreviewFailed(false);
          } else {
            setPreviewFailed(true);
            setErrorMessage('缩略图数据为空');
          }
        }
      } catch (error) {
        if (!cancelled) {
          setPreviewFailed(true);
          setErrorMessage(String(error));
        }
      } finally {
        if (!cancelled) {
          setIsLoading(false);
        }
      }
    }

    if (path) {
      loadThumbnail();
    } else {
      setPreviewFailed(true);
      setErrorMessage('路径为空');
      setIsLoading(false);
    }

    return () => {
      cancelled = true;
    };
  }, [path]);

  const canPreview = thumbnailData && !previewFailed;

  if (isMain) {
    // 主参考图 - 通栏显示
    return (
      <div className="w-full rounded-lg border border-surface bg-surface/30 p-3 space-y-2">
        <div className="flex items-center justify-between">
          <span className="text-[10px] text-blue-300 bg-blue-500/10 px-2 py-0.5 rounded">{badge}</span>
          {onReplace && (
            <button
              onClick={onReplace}
              className="flex items-center gap-1 text-[9px] text-text-secondary hover:text-text-primary transition-colors"
            >
              <RefreshCw size={12} />
              替换
            </button>
          )}
        </div>
        {isLoading ? (
          <div className="h-32 w-full rounded bg-bg-primary/80 border border-surface flex items-center justify-center">
            <div className="text-[10px] text-text-secondary/75">加载中...</div>
          </div>
        ) : canPreview ? (
          <img
            src={thumbnailData}
            alt={basename(path)}
            className="h-32 w-full rounded object-cover bg-bg-primary"
            onError={() => {
              setPreviewFailed(true);
              setErrorMessage('图片加载失败');
            }}
          />
        ) : (
          <div className="h-32 w-full rounded bg-bg-primary/80 border border-surface flex flex-col items-center justify-center text-[10px] text-text-secondary/75 text-center px-2 gap-1">
            <div>{basename(path)}</div>
            {errorMessage && <div className="text-[8px] text-red-400/80">错误: {errorMessage}</div>}
          </div>
        )}
        <div className="text-[8px] text-text-secondary/65 break-all">{path}</div>
      </div>
    );
  }

  // 辅助参考图 - 正方形小图
  return (
    <div className="relative rounded-lg border border-surface bg-surface/30 p-1.5 space-y-1 w-20 h-20">
      {onRemove && (
        <button
          onClick={onRemove}
          className="absolute -top-1 -right-1 w-4 h-4 rounded-full bg-red-500/80 hover:bg-red-500 flex items-center justify-center transition-colors z-10"
        >
          <X size={10} className="text-white" />
        </button>
      )}
      {isLoading ? (
        <div className="w-full h-full rounded bg-bg-primary/80 border border-surface flex items-center justify-center">
          <div className="text-[8px] text-text-secondary/75">...</div>
        </div>
      ) : canPreview ? (
        <img
          src={thumbnailData}
          alt={basename(path)}
          className="w-full h-full rounded object-cover bg-bg-primary"
          onError={() => {
            setPreviewFailed(true);
            setErrorMessage('加载失败');
          }}
        />
      ) : (
        <div className="w-full h-full rounded bg-bg-primary/80 border border-surface flex items-center justify-center text-[7px] text-text-secondary/75 text-center px-1">
          {errorMessage ? '错误' : basename(path)}
        </div>
      )}
    </div>
  );
}

interface UploadButtonProps {
  onUpload: () => void;
  label?: string;
}

function UploadButton({ onUpload, label = '上传' }: UploadButtonProps) {
  return (
    <button
      onClick={onUpload}
      className="w-20 h-20 rounded-lg border-2 border-dashed border-surface hover:border-blue-400/40 bg-surface/20 hover:bg-blue-500/10 flex flex-col items-center justify-center gap-1 transition-colors"
    >
      <Upload size={16} className="text-text-secondary" />
      <span className="text-[9px] text-text-secondary">{label}</span>
    </button>
  );
}

interface StyleTransferReferenceSelectionCardProps {
  auxReferencePaths: string[];
  mainReferencePath: string;
  onCancel(): void;
  onConfirm(styleTransferType: string): void;
  onUpdateMainReference(path: string): void;
  onUpdateAuxReferences(paths: string[]): void;
}

const STYLE_TRANSFER_TYPES = [
  { value: 'portrait', label: '人像', desc: '优化肤色和人物主体' },
  { value: 'landscape', label: '风光', desc: '优化天空、植被和自然场景' },
  { value: 'urban', label: '城市', desc: '优化建筑和城市夜景' },
  { value: 'general', label: '通用', desc: '适用于各类题材' },
] as const;

const IMAGE_EXTENSIONS = [
  'jpg',
  'jpeg',
  'png',
  'tiff',
  'tif',
  'webp',
  'bmp',
  'dng',
  'nef',
  'cr2',
  'cr3',
  'arw',
  'raf',
  'orf',
  'rw2',
];

export function StyleTransferReferenceSelectionCard({
  auxReferencePaths,
  mainReferencePath,
  onCancel,
  onConfirm,
  onUpdateMainReference,
  onUpdateAuxReferences,
}: StyleTransferReferenceSelectionCardProps) {
  const [selectedType, setSelectedType] = React.useState<string>('general');

  const handleUploadMainReference = async () => {
    try {
      const selected = await openFileDialog({
        multiple: false,
        filters: [
          {
            name: '图片文件',
            extensions: IMAGE_EXTENSIONS,
          },
        ],
      });

      if (selected && typeof selected === 'string') {
        onUpdateMainReference(selected);
      }
    } catch (error) {
      console.error('选择主参考图失败:', error);
    }
  };

  const handleUploadAuxReference = async () => {
    try {
      const selected = await openFileDialog({
        multiple: true,
        filters: [
          {
            name: '图片文件',
            extensions: IMAGE_EXTENSIONS,
          },
        ],
      });

      if (selected) {
        const selectedPaths = Array.isArray(selected) ? selected : [selected];
        const sanitizedPaths = selectedPaths.filter((path): path is string => typeof path === 'string' && !!path);
        if (sanitizedPaths.length > 0) {
          onUpdateAuxReferences([...auxReferencePaths, ...sanitizedPaths]);
        }
      }
    } catch (error) {
      console.error('选择辅助参考图失败:', error);
    }
  };

  const handleRemoveAuxReference = (index: number) => {
    const newPaths = auxReferencePaths.filter((_, i) => i !== index);
    onUpdateAuxReferences(newPaths);
  };

  const canConfirm = mainReferencePath && mainReferencePath.trim() !== '';

  return (
    <div className="w-full rounded-lg border border-blue-500/20 bg-blue-500/5 px-3 py-2.5 space-y-3">
      {/* 标题说明 */}
      <div className="space-y-0.5">
        <div className="text-[11px] font-medium text-text-primary">风格迁移 - 参考图设置</div>
        <div className="text-[9px] text-text-secondary/75">
          请上传主参考图（必需）和辅助参考图（可选），选择风格迁移类型后开始分析。
        </div>
      </div>

      {/* 风格迁移类型选择 */}
      <div className="space-y-1.5">
        <div className="text-[9px] text-text-secondary/80">风格迁移类型</div>
        <div className="grid grid-cols-2 gap-1.5">
          {STYLE_TRANSFER_TYPES.map((type) => (
            <button
              key={type.value}
              onClick={() => setSelectedType(type.value)}
              className={`rounded border px-2 py-1.5 text-left transition-colors ${
                selectedType === type.value
                  ? 'border-blue-400/40 bg-blue-500/15 text-blue-300'
                  : 'border-surface bg-surface/30 text-text-secondary hover:border-surface hover:bg-surface/50'
              }`}
            >
              <div className="text-[10px] font-medium">{type.label}</div>
              <div className="text-[8px] opacity-70">{type.desc}</div>
            </button>
          ))}
        </div>
      </div>

      {/* 主参考图 */}
      <div className="space-y-1.5">
        <div className="text-[9px] text-text-secondary/80">
          主参考图 <span className="text-red-400">*</span>
        </div>
        {mainReferencePath ? (
          <ReferencePreviewTile
            badge="主参考图"
            path={mainReferencePath}
            onReplace={handleUploadMainReference}
            isMain={true}
          />
        ) : (
          <button
            onClick={handleUploadMainReference}
            className="w-full h-32 rounded-lg border-2 border-dashed border-surface hover:border-blue-400/40 bg-surface/20 hover:bg-blue-500/10 flex flex-col items-center justify-center gap-2 transition-colors"
          >
            <Upload size={24} className="text-text-secondary" />
            <span className="text-[10px] text-text-secondary">点击上传主参考图</span>
            <span className="text-[8px] text-text-secondary/60">支持 JPG、PNG、RAW 等格式</span>
          </button>
        )}
      </div>

      {/* 辅助参考图 */}
      <div className="space-y-1.5">
        <div className="text-[9px] text-text-secondary/80">辅助参考图（可选）</div>
        <div className="flex flex-wrap gap-2">
          <UploadButton onUpload={handleUploadAuxReference} label="添加" />
          {auxReferencePaths.map((path, index) => (
            <ReferencePreviewTile
              key={`${path}-${index}`}
              badge={`辅助 ${index + 1}`}
              path={path}
              onRemove={() => handleRemoveAuxReference(index)}
            />
          ))}
        </div>
      </div>

      {/* 操作按钮 */}
      <div className="flex items-center justify-end gap-2 pt-1">
        <button
          onClick={onCancel}
          className="rounded px-3 py-1.5 text-[10px] text-text-secondary hover:text-text-primary hover:bg-surface transition-colors"
        >
          取消
        </button>
        <button
          onClick={() => onConfirm(selectedType)}
          disabled={!canConfirm}
          className={`rounded px-3 py-1.5 text-[10px] transition-colors ${
            canConfirm
              ? 'bg-blue-500/20 text-blue-300 hover:bg-blue-500/30'
              : 'bg-surface/30 text-text-secondary/50 cursor-not-allowed'
          }`}
        >
          确认并开始分析
        </button>
      </div>
    </div>
  );
}
