import React, { useMemo, useState } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';

interface ReferencePreviewTileProps {
  badge: string;
  path: string;
}

function isPreviewableReference(path: string) {
  const ext = path.split('.').pop()?.toLowerCase() || '';
  // 支持常见图片格式和RAW格式
  return [
    'jpg',
    'jpeg',
    'png',
    'webp',
    'bmp',
    'gif',
    'tif',
    'tiff',
    'dng',
    'nef',
    'cr2',
    'cr3',
    'arw',
    'raf',
    'orf',
    'rw2',
  ].includes(ext);
}

function basename(path: string) {
  return path.split(/[\\/]/).pop() || path;
}

function ReferencePreviewTile({ badge, path }: ReferencePreviewTileProps) {
  const [previewFailed, setPreviewFailed] = useState(false);
  const previewSrc = useMemo(() => convertFileSrc(path), [path]);
  const canPreview = isPreviewableReference(path) && !previewFailed;

  return (
    <div className="rounded-lg border border-surface bg-surface/30 p-2 space-y-1">
      <div className="flex items-center justify-between gap-2">
        <span className="text-[9px] text-blue-300 bg-blue-500/10 px-1.5 py-0.5 rounded">{badge}</span>
        <span className="text-[9px] text-text-secondary truncate">{basename(path)}</span>
      </div>
      {canPreview ? (
        <img
          src={previewSrc}
          alt={basename(path)}
          className="h-20 w-full rounded object-cover bg-bg-primary"
          onError={() => setPreviewFailed(true)}
        />
      ) : (
        <div className="h-20 w-full rounded bg-bg-primary/80 border border-surface flex items-center justify-center text-[9px] text-text-secondary/75 text-center px-2">
          {basename(path)}
        </div>
      )}
      <div className="text-[9px] text-text-secondary/65 break-all">{path}</div>
    </div>
  );
}

interface StyleTransferReferenceSelectionCardProps {
  auxReferencePaths: string[];
  mainReferencePath: string;
  onCancel(): void;
  onConfirm(styleTransferType: string): void;
}

const STYLE_TRANSFER_TYPES = [
  { value: 'portrait', label: '人像', desc: '优化肤色和人物主体' },
  { value: 'landscape', label: '风光', desc: '优化天空、植被和自然场景' },
  { value: 'urban', label: '城市', desc: '优化建筑和城市夜景' },
  { value: 'general', label: '通用', desc: '适用于各类题材' },
] as const;

export function StyleTransferReferenceSelectionCard({
  auxReferencePaths,
  mainReferencePath,
  onCancel,
  onConfirm,
}: StyleTransferReferenceSelectionCardProps) {
  const [selectedType, setSelectedType] = React.useState<string>('general');

  return (
    <div className="w-full rounded-lg border border-blue-500/20 bg-blue-500/5 px-2.5 py-2 space-y-2">
      <div className="space-y-0.5">
        <div className="text-[10px] text-text-primary">参考图确认</div>
        <div className="text-[9px] text-text-secondary/75">
          请确认主参考图和辅助参考图无误，并选择风格迁移类型。确认后才会开始风格迁移分析。
        </div>
      </div>

      {/* 风格迁移类型选择 */}
      <div className="space-y-1">
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

      <ReferencePreviewTile badge="主参考图" path={mainReferencePath} />
      {auxReferencePaths.length > 0 && (
        <div className="space-y-1">
          <div className="text-[9px] text-text-secondary/80">辅助参考图</div>
          <div className="grid grid-cols-1 gap-2">
            {auxReferencePaths.map((path, index) => (
              <ReferencePreviewTile key={`${path}-${index}`} badge={`辅助 ${index + 1}`} path={path} />
            ))}
          </div>
        </div>
      )}
      <div className="flex items-center justify-end gap-2">
        <button
          onClick={onCancel}
          className="rounded px-2 py-1 text-[10px] text-text-secondary hover:text-text-primary hover:bg-surface transition-colors"
        >
          取消
        </button>
        <button
          onClick={() => onConfirm(selectedType)}
          className="rounded px-2 py-1 text-[10px] bg-blue-500/20 text-blue-300 hover:bg-blue-500/30 transition-colors"
        >
          确认并开始分析
        </button>
      </div>
    </div>
  );
}
