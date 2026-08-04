import { AlertTriangle, Loader2, RotateCcw, Scan, ZoomIn, ZoomOut } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useSmartCullingText } from '../i18n';
import type { DetectedFace } from '../types';
import { useRenderedPreview } from './useRenderedPreview';

const MIN_ZOOM = 0.25;
const MAX_ZOOM = 16;

export interface EvidenceViewState {
  zoom: number;
  /** Pan expressed as a fraction of the rendered image size so mixed orientations stay aligned. */
  pan: { x: number; y: number };
}

const FIT_VIEW: EvidenceViewState = { zoom: 1, pan: { x: 0, y: 0 } };

interface PhotoEvidenceViewportProps {
  path: string;
  fallbackUrl?: string;
  alt: string;
  faces?: DetectedFace[];
  selectedFaceKeys?: ReadonlySet<string>;
  faceLabels?: ReadonlyMap<string, string>;
  onFaceClick?: (face: DetectedFace) => void;
  onPrevious?: () => void;
  onNext?: () => void;
  viewState?: EvidenceViewState;
  onViewStateChange?: (view: EvidenceViewState) => void;
  compact?: boolean;
}

export function faceSelectionKey(path: string, bbox: DetectedFace['bbox']) {
  return `${path}:${bbox.join(',')}`;
}

export function PhotoEvidenceViewport({
  path,
  fallbackUrl,
  alt,
  faces = [],
  selectedFaceKeys,
  faceLabels,
  onFaceClick,
  onPrevious,
  onNext,
  viewState,
  onViewStateChange,
  compact = false,
}: PhotoEvidenceViewportProps) {
  const tx = useSmartCullingText();
  const { loadedUrl, loading, error, retry } = useRenderedPreview(path, fallbackUrl);
  const [failedSources, setFailedSources] = useState<ReadonlySet<string>>(() => new Set());
  const [naturalSize, setNaturalSize] = useState({ width: 0, height: 0 });
  const [containerSize, setContainerSize] = useState({ width: 0, height: 0 });
  const [internalView, setInternalView] = useState<EvidenceViewState>(FIT_VIEW);
  const isControlled = viewState !== undefined;
  const containerRef = useRef<HTMLDivElement>(null);
  const view = viewState ?? internalView;
  const viewRef = useRef(view);
  const dragRef = useRef<{
    pointerId: number;
    origin: { x: number; y: number };
    pan: { x: number; y: number };
  } | null>(null);

  useEffect(() => {
    viewRef.current = view;
  }, [view]);

  const updateView = useCallback(
    (next: EvidenceViewState) => {
      const normalized = {
        zoom: Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, next.zoom)),
        pan: next.pan,
      };
      viewRef.current = normalized;
      if (onViewStateChange) onViewStateChange(normalized);
      else setInternalView(normalized);
    },
    [onViewStateChange],
  );

  useEffect(() => {
    if (!isControlled) setInternalView(FIT_VIEW);
  }, [isControlled, path]);

  useEffect(() => {
    const element = containerRef.current;
    if (!element) return;
    const measure = () => setContainerSize({ width: element.clientWidth, height: element.clientHeight });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const fitScale = useMemo(() => {
    if (!naturalSize.width || !naturalSize.height || !containerSize.width || !containerSize.height) return 1;
    return Math.min(containerSize.width / naturalSize.width, containerSize.height / naturalSize.height);
  }, [containerSize, naturalSize]);
  const absoluteScale = fitScale * view.zoom;
  const renderedSize = {
    width: naturalSize.width * absoluteScale,
    height: naturalSize.height * absoluteScale,
  };
  const origin = {
    x: (containerSize.width - renderedSize.width) / 2 + view.pan.x * renderedSize.width,
    y: (containerSize.height - renderedSize.height) / 2 + view.pan.y * renderedSize.height,
  };
  const sourceUrl = [loadedUrl, fallbackUrl].find((source) => source && !failedSources.has(source));

  useEffect(() => {
    setNaturalSize({ width: 0, height: 0 });
  }, [path, sourceUrl]);

  const zoomAt = useCallback(
    (nextZoom: number, point = { x: containerSize.width / 2, y: containerSize.height / 2 }) => {
      const current = viewRef.current;
      const clampedZoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, nextZoom));
      const baseWidth = naturalSize.width * fitScale;
      const baseHeight = naturalSize.height * fitScale;
      const currentWidth = baseWidth * current.zoom;
      const currentHeight = baseHeight * current.zoom;
      const nextWidth = baseWidth * clampedZoom;
      const nextHeight = baseHeight * clampedZoom;
      if (!currentWidth || !currentHeight || !nextWidth || !nextHeight) {
        updateView({ zoom: clampedZoom, pan: current.pan });
        return;
      }
      const currentOrigin = {
        x: (containerSize.width - currentWidth) / 2 + current.pan.x * currentWidth,
        y: (containerSize.height - currentHeight) / 2 + current.pan.y * currentHeight,
      };
      const sourcePoint = {
        x: (point.x - currentOrigin.x) / currentWidth,
        y: (point.y - currentOrigin.y) / currentHeight,
      };
      updateView({
        zoom: clampedZoom,
        pan: {
          x: (point.x - sourcePoint.x * nextWidth - (containerSize.width - nextWidth) / 2) / nextWidth,
          y: (point.y - sourcePoint.y * nextHeight - (containerSize.height - nextHeight) / 2) / nextHeight,
        },
      });
    },
    [containerSize, fitScale, naturalSize, updateView],
  );

  const toggleActualSize = () => {
    const actualZoom = fitScale > 0 ? 1 / fitScale : 1;
    if (Math.abs(viewRef.current.zoom - actualZoom) < 0.05) updateView(FIT_VIEW);
    else updateView({ zoom: actualZoom, pan: { x: 0, y: 0 } });
  };

  return (
    <div
      ref={containerRef}
      className={`sc-evidence-viewport ${compact ? 'is-compact' : ''}`}
      tabIndex={0}
      onDoubleClick={toggleActualSize}
      onWheel={(event) => {
        event.preventDefault();
        const rect = event.currentTarget.getBoundingClientRect();
        zoomAt(viewRef.current.zoom * Math.exp(-event.deltaY * 0.002), {
          x: event.clientX - rect.left,
          y: event.clientY - rect.top,
        });
      }}
      onPointerDown={(event) => {
        if (event.button !== 0 || (event.target as HTMLElement).closest('button')) return;
        event.currentTarget.focus();
        event.currentTarget.setPointerCapture(event.pointerId);
        dragRef.current = {
          pointerId: event.pointerId,
          origin: { x: event.clientX, y: event.clientY },
          pan: viewRef.current.pan,
        };
      }}
      onPointerMove={(event) => {
        const drag = dragRef.current;
        if (!drag || drag.pointerId !== event.pointerId) return;
        updateView({
          ...viewRef.current,
          pan: {
            x: drag.pan.x + (event.clientX - drag.origin.x) / Math.max(renderedSize.width, 1),
            y: drag.pan.y + (event.clientY - drag.origin.y) / Math.max(renderedSize.height, 1),
          },
        });
      }}
      onPointerUp={(event) => {
        if (dragRef.current?.pointerId === event.pointerId) dragRef.current = null;
      }}
      onKeyDown={(event) => {
        if (event.key === '+' || event.key === '=') {
          event.preventDefault();
          event.stopPropagation();
          zoomAt(viewRef.current.zoom * 1.25);
        } else if (event.key === '-') {
          event.preventDefault();
          event.stopPropagation();
          zoomAt(viewRef.current.zoom / 1.25);
        } else if (event.key === '0') {
          event.preventDefault();
          event.stopPropagation();
          updateView(FIT_VIEW);
        } else if (event.key === 'Enter') {
          event.preventDefault();
          event.stopPropagation();
          toggleActualSize();
        } else if (event.key === 'ArrowLeft' && onPrevious) {
          event.preventDefault();
          event.stopPropagation();
          onPrevious();
        } else if (event.key === 'ArrowRight' && onNext) {
          event.preventDefault();
          event.stopPropagation();
          onNext();
        }
      }}
    >
      {sourceUrl ? (
        <img
          className="sc-evidence-image"
          src={sourceUrl}
          alt={alt}
          draggable={false}
          style={{
            left: origin.x,
            top: origin.y,
            width: renderedSize.width,
            height: renderedSize.height,
          }}
          onLoad={(event) =>
            setNaturalSize({
              width: event.currentTarget.naturalWidth,
              height: event.currentTarget.naturalHeight,
            })
          }
          onError={() => {
            const failedUrl = sourceUrl;
            if (failedUrl) setFailedSources((current) => new Set(current).add(failedUrl));
            setNaturalSize({ width: 0, height: 0 });
          }}
        />
      ) : (
        <div className="sc-evidence-empty">{alt}</div>
      )}
      {sourceUrl && naturalSize.width
        ? faces.map((face, index) => {
            const key = faceSelectionKey(path, face.bbox);
            const faceLabel = faceLabels?.get(key) ?? String(index + 1);
            const faceStyle = {
              left: origin.x + face.bbox[0] * renderedSize.width,
              top: origin.y + face.bbox[1] * renderedSize.height,
              width: face.bbox[2] * renderedSize.width,
              height: face.bbox[3] * renderedSize.height,
            };
            if (!onFaceClick) {
              return (
                <div
                  key={`${key}-${index}`}
                  className="sc-evidence-face is-passive"
                  style={faceStyle}
                  aria-hidden="true"
                >
                  <span>{faceLabel}</span>
                </div>
              );
            }
            return (
              <button
                key={`${key}-${index}`}
                className={`sc-evidence-face ${selectedFaceKeys?.has(key) ? 'is-selected' : ''}`}
                style={faceStyle}
                onDoubleClick={(event) => event.stopPropagation()}
                onClick={() => onFaceClick(face)}
                aria-label={faceLabels?.has(key) ? `${tx('selectPerson')} ${faceLabel}` : `${index + 1}`}
              >
                <span>{faceLabel}</span>
              </button>
            );
          })
        : null}
      {loading ? (
        <span className="sc-evidence-loading">
          <Loader2 size={15} className="animate-spin" />
        </span>
      ) : null}
      {error || failedSources.size > 0 ? (
        <div className="sc-evidence-error" role="alert">
          <AlertTriangle size={15} />
          <span>
            {fallbackUrl && sourceUrl === fallbackUrl ? tx('previewUsingThumbnail') : tx('previewUnavailable')}
          </span>
          <button
            onClick={() => {
              setFailedSources(new Set());
              retry();
            }}
          >
            <RotateCcw size={13} />
            {tx('retryPreview')}
          </button>
        </div>
      ) : null}
      <div className="sc-evidence-toolbar">
        <button title={tx('zoomOut')} aria-label={tx('zoomOut')} onClick={() => zoomAt(viewRef.current.zoom / 1.25)}>
          <ZoomOut size={14} />
        </button>
        <button title={tx('fitImage')} aria-label={tx('fitImage')} onClick={() => updateView(FIT_VIEW)}>
          <Scan size={14} />
        </button>
        <button
          className="sc-evidence-actual"
          title={tx('actualSize')}
          aria-label={tx('actualSize')}
          onClick={toggleActualSize}
        >
          1:1
        </button>
        <button title={tx('zoomIn')} aria-label={tx('zoomIn')} onClick={() => zoomAt(viewRef.current.zoom * 1.25)}>
          <ZoomIn size={14} />
        </button>
        <span>{Math.round(absoluteScale * 100)}%</span>
      </div>
    </div>
  );
}
