import { motion } from 'framer-motion';
import clsx from 'clsx';
import Slider from '../ui/Slider';
import { Adjustments, BasicAdjustment } from '../../utils/adjustments';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

interface BasicAdjustmentsProps {
  adjustments: Adjustments;
  setAdjustments(adjustments: Adjustments | ((prev: Adjustments) => Adjustments)): void;
  isForMask?: boolean;
  onDragStateChange?: (isDragging: boolean) => void;
}

type TFunction = (key: string) => string;

const getToneMapperOptions = (t: TFunction) => [
  { id: 'basic', label: t('adjustments.basic'), title: t('adjustments.standardTonemapping') },
  { id: 'agx', label: t('adjustments.agx'), title: t('adjustments.filmLikeTonemapping') },
];

interface ToneMapperSwitchProps {
  selectedMapper: string;
  onMapperChange: (mapper: string) => void;
  exposureValue: number;
  onExposureChange: (value: number) => void;
  onDragStateChange?: (isDragging: boolean) => void;
}

const ToneMapperSwitch = ({
  selectedMapper,
  onMapperChange,
  exposureValue,
  onExposureChange,
  onDragStateChange,
}: ToneMapperSwitchProps) => {
  const { t } = useTranslation();
  const toneMapperOptions = getToneMapperOptions(t);
  const [buttonRefs, setButtonRefs] = useState<Map<string, HTMLButtonElement>>(new Map());
  const [bubbleStyle, setBubbleStyle] = useState({});
  const containerRef = useRef<HTMLDivElement>(null);
  const isInitialAnimation = useRef(true);
  const [isLabelHovered, setIsLabelHovered] = useState(false);

  const handleReset = () => {
    onMapperChange('basic');
    onExposureChange(0);
  };

  useEffect(() => {
    const selectedButton = buttonRefs.get(selectedMapper);

    if (selectedButton && containerRef.current) {
      const targetStyle = {
        x: selectedButton.offsetLeft,
        width: selectedButton.offsetWidth,
      };

      if (isInitialAnimation.current && containerRef.current.offsetWidth > 0) {
        let initialX;
        if (selectedMapper === 'agx') {
          initialX = containerRef.current.offsetWidth;
        } else {
          initialX = -targetStyle.width;
        }

        setBubbleStyle({
          x: [initialX, targetStyle.x],
          width: targetStyle.width,
        });
        isInitialAnimation.current = false;
      } else {
        setBubbleStyle(targetStyle);
      }
    }
  }, [selectedMapper, buttonRefs]);

  return (
    <div className="group">
      <div className="flex justify-between items-center mb-2">
        <div
          className="grid cursor-pointer"
          onClick={handleReset}
          onDoubleClick={handleReset}
          onMouseEnter={() => setIsLabelHovered(true)}
          onMouseLeave={() => setIsLabelHovered(false)}
        >
          <span
            aria-hidden={isLabelHovered}
            className={`col-start-1 row-start-1 text-sm font-medium text-text-secondary select-none transition-opacity duration-200 ease-in-out ${
              isLabelHovered ? 'opacity-0' : 'opacity-100'
            }`}
          >
            {t('adjustments.toneMapper')}
          </span>
          <span
            aria-hidden={!isLabelHovered}
            className={`col-start-1 row-start-1 text-sm font-medium text-text-primary select-none transition-opacity duration-200 ease-in-out pointer-events-none ${
              isLabelHovered ? 'opacity-100' : 'opacity-0'
            }`}
          >
            {t('common.reset')}
          </span>
        </div>
      </div>
      <div className="w-full p-2 pb-1 bg-card-active rounded-md">
        <div ref={containerRef} className="relative flex w-full">
          <motion.div
            className="absolute top-0 bottom-0 z-0 bg-accent"
            style={{ borderRadius: 6 }}
            animate={bubbleStyle}
            transition={{ type: 'spring', bounce: 0.2, duration: 0.6 }}
          />
          {toneMapperOptions.map((mapper) => (
            <button
              key={mapper.id}
              data-tooltip={mapper.title}
              ref={(el) => {
                if (el) {
                  const newRefs = new Map(buttonRefs);
                  if (newRefs.get(mapper.id) !== el) {
                    newRefs.set(mapper.id, el);
                    setButtonRefs(newRefs);
                  }
                }
              }}
              onClick={() => onMapperChange(mapper.id)}
              className={clsx(
                'relative flex-1 flex items-center justify-center gap-2 px-3 p-1.5 text-sm font-medium rounded-md transition-colors',
                {
                  'text-text-primary hover:bg-surface': selectedMapper !== mapper.id,
                  'text-button-text': selectedMapper === mapper.id,
                },
              )}
              style={{ WebkitTapHighlightColor: 'transparent' }}
            >
              <span className="relative z-10 flex items-center">{mapper.label}</span>
            </button>
          ))}
        </div>
        <div className="mt-2.5 px-1">
          <Slider
            label={t('adjustments.exposure')}
            max={5}
            min={-5}
            onChange={(e: { target: { value: number | string } }) =>
              onExposureChange(parseFloat(String(e.target.value)))
            }
            step={0.01}
            value={exposureValue}
            trackClassName="bg-surface"
            onDragStateChange={onDragStateChange}
          />
        </div>
      </div>
    </div>
  );
};

export default function BasicAdjustments({
  adjustments,
  setAdjustments,
  isForMask = false,
  onDragStateChange,
}: BasicAdjustmentsProps) {
  const { t } = useTranslation();
  const handleAdjustmentChange = (key: BasicAdjustment, value: number | string) => {
    const numericValue = parseFloat(String(value));
    setAdjustments((prev: Adjustments) => ({
      ...prev,
      [key]: numericValue,
    }));
  };

  const handleToneMapperChange = (mapper: string) => {
    setAdjustments((prev: Adjustments) => ({
      ...prev,
      toneMapper: mapper as 'basic' | 'agx',
    }));
  };

  return (
    <div>
      <Slider
        label={t('adjustments.brightness')}
        max={5}
        min={-5}
        onChange={(e: { target: { value: number | string } }) =>
          handleAdjustmentChange(BasicAdjustment.Brightness, e.target.value)
        }
        step={0.01}
        value={adjustments.brightness}
        onDragStateChange={onDragStateChange}
      />
      <Slider
        label={t('adjustments.contrast')}
        max={100}
        min={-100}
        onChange={(e: { target: { value: number | string } }) =>
          handleAdjustmentChange(BasicAdjustment.Contrast, e.target.value)
        }
        step={1}
        value={adjustments.contrast}
        onDragStateChange={onDragStateChange}
      />
      <Slider
        label={t('adjustments.highlights')}
        max={100}
        min={-100}
        onChange={(e: { target: { value: number | string } }) =>
          handleAdjustmentChange(BasicAdjustment.Highlights, e.target.value)
        }
        step={1}
        value={adjustments.highlights}
        onDragStateChange={onDragStateChange}
      />
      <Slider
        label={t('adjustments.shadows')}
        max={100}
        min={-100}
        onChange={(e: { target: { value: number | string } }) =>
          handleAdjustmentChange(BasicAdjustment.Shadows, e.target.value)
        }
        step={1}
        value={adjustments.shadows}
        onDragStateChange={onDragStateChange}
      />
      <Slider
        label={t('adjustments.whites')}
        max={100}
        min={-100}
        onChange={(e: { target: { value: number | string } }) =>
          handleAdjustmentChange(BasicAdjustment.Whites, e.target.value)
        }
        step={1}
        value={adjustments.whites}
        onDragStateChange={onDragStateChange}
      />
      <Slider
        label={t('adjustments.blacks')}
        max={100}
        min={-100}
        onChange={(e: { target: { value: number | string } }) =>
          handleAdjustmentChange(BasicAdjustment.Blacks, e.target.value)
        }
        step={1}
        value={adjustments.blacks}
        onDragStateChange={onDragStateChange}
      />

      {isForMask ? (
        <Slider
          label={t('adjustments.exposure')}
          max={5}
          min={-5}
          onChange={(e: { target: { value: number | string } }) =>
            handleAdjustmentChange(BasicAdjustment.Exposure, e.target.value)
          }
          step={0.01}
          value={adjustments.exposure}
          onDragStateChange={onDragStateChange}
        />
      ) : (
        <ToneMapperSwitch
          selectedMapper={adjustments.toneMapper || 'agx'}
          onMapperChange={handleToneMapperChange}
          exposureValue={adjustments.exposure}
          onExposureChange={(value) => handleAdjustmentChange(BasicAdjustment.Exposure, value)}
          onDragStateChange={onDragStateChange}
        />
      )}
    </div>
  );
}
