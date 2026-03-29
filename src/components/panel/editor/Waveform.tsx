import { useState, useEffect, useRef, type RefObject } from 'react';
import Draggable from 'react-draggable';
import { X, Waves } from 'lucide-react';
import { motion } from 'framer-motion';
import { useTranslation } from 'react-i18next';
import { WaveformData } from '../../ui/AppProperties';
import { DisplayMode } from '../../../utils/adjustments';
import Text from '../../ui/Text';
import { TextVariants } from '../../../types/typography';

interface WaveformProps {
  onClose(): void;
  waveformData: WaveformData;
}

const modeButtons = [
  { mode: DisplayMode.Luma, label: 'L' },
  { mode: DisplayMode.Rgb, label: 'RGB' },
  { mode: DisplayMode.Parade, label: 'P' },
  { mode: DisplayMode.Vectorscope, label: 'V' },
];

const useRawRgbaCanvas = (
  canvasRef: RefObject<HTMLCanvasElement | null>,
  base64Data: string,
  width: number,
  height: number,
) => {
  useEffect(() => {
    if (!base64Data || !canvasRef.current || !width || !height) {
      return;
    }

    const ctx = canvasRef.current.getContext('2d');
    if (!ctx) {
      return;
    }

    const binary = atob(base64Data);
    const bytes = new Uint8ClampedArray(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    const imageData = new ImageData(bytes, width, height);
    ctx.putImageData(imageData, 0, 0);
  }, [canvasRef, base64Data, width, height]);
};

export default function Waveform({ waveformData, onClose }: WaveformProps) {
  const [displayMode, setDisplayMode] = useState<DisplayMode>(DisplayMode.Rgb);
  const nodeRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const { t } = useTranslation();

  const width = waveformData?.width || 256;
  const height = waveformData?.height || 256;

  const activeData =
    displayMode === DisplayMode.Luma
      ? waveformData?.luma
      : displayMode === DisplayMode.Parade
        ? waveformData?.parade
        : displayMode === DisplayMode.Vectorscope
          ? waveformData?.vectorscope
          : waveformData?.rgb;

  useRawRgbaCanvas(canvasRef, activeData || '', width, height);

  const baseButtonClass =
    'flex-grow text-center px-2 py-1 text-xs rounded-lg font-medium transition-colors duration-150';
  const inactiveButtonClass = 'text-text-primary hover:bg-bg-tertiary';

  return (
    <Draggable nodeRef={nodeRef} handle=".handle" bounds="parent">
      <div ref={nodeRef} className="absolute top-20 left-20 z-50">
        <motion.div
          animate={{ opacity: 1, scale: 1 }}
          className="bg-surface/90 backdrop-blur-md border border-text-secondary/10 shadow-xl rounded-lg overflow-hidden"
          exit={{ opacity: 0, scale: 0.95 }}
          initial={{ opacity: 0, scale: 0.95 }}
          key="waveform-content"
          style={{ transformOrigin: 'top left' }}
          transition={{ duration: 0.2, ease: 'easeOut' }}
        >
          <div className="handle flex items-center justify-between p-3 cursor-move">
            <div className="flex items-center gap-2">
              <Waves size={16} />
              <Text variant={TextVariants.heading}>{t('waveform.waveform')}</Text>
            </div>
            <button
              className="p-1 rounded-lg text-text-secondary hover:bg-bg-primary hover:text-text-primary transition-colors"
              onClick={onClose}
            >
              <X size={16} />
            </button>
          </div>

          <div className="p-2 pt-0">
            <div className="relative w-[256px] h-[256px] bg-black/60 rounded overflow-hidden">
              {!!activeData && (
                <canvas
                  ref={canvasRef}
                  width={width}
                  height={height}
                  className="absolute inset-0 w-full h-full"
                  style={{ imageRendering: 'pixelated' }}
                />
              )}
            </div>
            <div className="flex justify-center gap-1 mt-2 p-1 bg-bg-primary rounded-lg">
              {modeButtons.map((item) => (
                <button
                  key={item.mode}
                  onClick={() => setDisplayMode(item.mode)}
                  className={`${baseButtonClass} ${
                    displayMode === item.mode ? 'bg-accent text-button-text' : inactiveButtonClass
                  }`}
                >
                  {item.label}
                </button>
              ))}
            </div>
          </div>
        </motion.div>
      </div>
    </Draggable>
  );
}
