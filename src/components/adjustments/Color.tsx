import { useState } from 'react';
import { Pipette } from 'lucide-react';
import Slider from '../ui/Slider';
import ColorWheel from '../ui/ColorWheel';
import { ColorAdjustment, ColorCalibration, HueSatLum, INITIAL_ADJUSTMENTS } from '../../utils/adjustments';
import { Adjustments, ColorGrading } from '../../utils/adjustments';
import { AppSettings } from '../ui/AppProperties';
import Text from '../ui/Text';
import { TextColors, TextVariants, TextWeights } from '../../types/typography';
import { useTranslation } from 'react-i18next';

interface ColorProps {
  color: string;
  name: string;
}

interface ColorPanelProps {
  adjustments: Adjustments;
  setAdjustments(adjustments: Adjustments | ((prev: Adjustments) => Adjustments)): void;
  appSettings: AppSettings | null;
  isForMask?: boolean;
  isWbPickerActive?: boolean;
  toggleWbPicker?: () => void;
  onDragStateChange?: (isDragging: boolean) => void;
}

interface ColorSwatchProps {
  color: string;
  isActive: boolean;
  name: string;
  onClick: (name: string) => void;
}

const HSL_COLORS: Array<ColorProps> = [
  { name: 'reds', color: '#f87171' },
  { name: 'oranges', color: '#fb923c' },
  { name: 'yellows', color: '#facc15' },
  { name: 'greens', color: '#4ade80' },
  { name: 'aquas', color: '#2dd4bf' },
  { name: 'blues', color: '#60a5fa' },
  { name: 'purples', color: '#a78bfa' },
  { name: 'magentas', color: '#f472b6' },
];

const ColorSwatch = ({ color, name, isActive, onClick }: ColorSwatchProps) => {
  const [isPressed, setIsPressed] = useState(false);
  const [isHovered, setIsHovered] = useState(false);

  const handleMouseDown = () => {
    setIsPressed(true);
  };

  const handleMouseUp = () => {
    setIsPressed(false);
  };

  const handleMouseLeave = () => {
    setIsPressed(false);
    setIsHovered(false);
  };

  const handleMouseEnter = () => {
    setIsHovered(true);
  };

  const handleClick = () => {
    onClick(name);
  };

  const getTransform = () => {
    if (isPressed) return 'scale(0.95)';
    if (isActive) return 'scale(1.1)';
    if (isHovered) return 'scale(1.08)';
    return 'scale(1)';
  };

  return (
    <button
      aria-label={`Select ${name} color`}
      className="relative w-6 h-6 focus:outline-none group"
      onClick={handleClick}
      onMouseDown={handleMouseDown}
      onMouseUp={handleMouseUp}
      onMouseLeave={handleMouseLeave}
      onMouseEnter={handleMouseEnter}
      onTouchStart={handleMouseDown}
      onTouchEnd={handleMouseUp}
    >
      <div
        className={`absolute inset-0 rounded-full border-2 transition-all duration-200 ease-out ${
          isActive ? 'border-white opacity-100' : 'scale-100 border-transparent opacity-0'
        }`}
        style={{
          transform: isActive ? (isPressed ? 'scale(1.1)' : 'scale(1.25)') : undefined,
          transition: isPressed
            ? 'transform 100ms cubic-bezier(0.4, 0, 0.2, 1), opacity 200ms ease-out'
            : 'transform 200ms cubic-bezier(0.34, 1.56, 0.64, 1), opacity 200ms ease-out',
        }}
      />

      <div
        className={`absolute inset-0 rounded-full transition-all duration-150 ease-out ${
          isActive ? 'shadow-lg' : 'shadow-md'
        }`}
        style={{
          backgroundColor: color,
          transform: getTransform(),
          transition: isPressed
            ? 'transform 100ms cubic-bezier(0.4, 0, 0.2, 1)'
            : 'transform 200ms cubic-bezier(0.34, 1.56, 0.64, 1)',
        }}
      />
    </button>
  );
};

const ColorGradingPanel = ({ adjustments, setAdjustments, onDragStateChange }: ColorPanelProps) => {
  const { t } = useTranslation();
  const colorGrading = adjustments.colorGrading || INITIAL_ADJUSTMENTS.colorGrading;

  const handleChange = (grading: ColorGrading, newValue: HueSatLum) => {
    setAdjustments((prev: Adjustments) => ({
      ...prev,
      colorGrading: {
        ...(prev.colorGrading || INITIAL_ADJUSTMENTS.colorGrading),
        [grading]: newValue,
      },
    }));
  };

  const handleGlobalChange = (grading: ColorGrading, value: string) => {
    setAdjustments((prev: Adjustments) => ({
      ...prev,
      colorGrading: {
        ...(prev.colorGrading || INITIAL_ADJUSTMENTS.colorGrading),
        [grading]: parseFloat(value),
      },
    }));
  };

  return (
    <div>
      <div className="flex justify-center mb-4">
        <div className="w-[calc(50%-0.5rem)]">
          <ColorWheel
            defaultValue={INITIAL_ADJUSTMENTS.colorGrading.midtones}
            label={t('adjustments.midtones')}
            onChange={(val: HueSatLum) => handleChange(ColorGrading.Midtones, val)}
            value={colorGrading.midtones}
            onDragStateChange={onDragStateChange}
          />
        </div>
      </div>
      <div className="flex justify-between mb-2 gap-4">
        <div className="w-full">
          <ColorWheel
            defaultValue={INITIAL_ADJUSTMENTS.colorGrading.shadows}
            label={t('adjustments.shadows')}
            onChange={(val: HueSatLum) => handleChange(ColorGrading.Shadows, val)}
            value={colorGrading.shadows}
            onDragStateChange={onDragStateChange}
          />
        </div>
        <div className="w-full">
          <ColorWheel
            defaultValue={INITIAL_ADJUSTMENTS.colorGrading.highlights}
            label={t('adjustments.highlights')}
            onChange={(val: HueSatLum) => handleChange(ColorGrading.Highlights, val)}
            value={colorGrading.highlights}
            onDragStateChange={onDragStateChange}
          />
        </div>
      </div>
      <div>
        <Slider
          defaultValue={50}
          label={t('adjustments.blending')}
          max={100}
          min={0}
          onChange={(e: { target: { value: number | string } }) =>
            handleGlobalChange(ColorGrading.Blending, String(e.target.value))
          }
          step={1}
          value={colorGrading.blending}
          onDragStateChange={onDragStateChange}
        />
        <Slider
          defaultValue={0}
          label={t('adjustments.balance')}
          max={100}
          min={-100}
          onChange={(e: { target: { value: number | string } }) =>
            handleGlobalChange(ColorGrading.Balance, String(e.target.value))
          }
          step={1}
          value={colorGrading.balance}
          onDragStateChange={onDragStateChange}
        />
      </div>
    </div>
  );
};

const ColorCalibrationPanel = ({ adjustments, setAdjustments, onDragStateChange }: ColorPanelProps) => {
  const { t } = useTranslation();
  const [activePrimary, setActivePrimary] = useState('red');
  const colorCalibration = adjustments.colorCalibration || INITIAL_ADJUSTMENTS.colorCalibration;

  const PRIMARY_COLORS = [
    { name: 'red', color: '#f87171' },
    { name: 'green', color: '#4ade80' },
    { name: 'blue', color: '#60a5fa' },
  ];

  const handleShadowsChange = (value: string) => {
    setAdjustments((prev: Adjustments) => ({
      ...prev,
      colorCalibration: {
        ...(prev.colorCalibration || INITIAL_ADJUSTMENTS.colorCalibration),
        shadowsTint: parseFloat(value),
      },
    }));
  };

  const handlePrimaryChange = (key: 'Hue' | 'Saturation', value: string) => {
    const fullKey = `${activePrimary}${key}` as keyof ColorCalibration;
    setAdjustments((prev: Adjustments) => ({
      ...prev,
      colorCalibration: {
        ...(prev.colorCalibration || INITIAL_ADJUSTMENTS.colorCalibration),
        [fullKey]: parseFloat(value),
      },
    }));
  };

  const currentValues = {
    hue: colorCalibration[`${activePrimary}Hue` as keyof ColorCalibration] || 0,
    saturation: colorCalibration[`${activePrimary}Saturation` as keyof ColorCalibration] || 0,
  };

  return (
    <div className="p-2 bg-bg-tertiary rounded-md mt-4">
      <Text variant={TextVariants.heading} className="mb-2">
        {t('adjustments.colorCalibration')}
      </Text>
      <div>
        <Text color={TextColors.primary} weight={TextWeights.medium} className="mb-1">
          {t('adjustments.shadows')}
        </Text>
        <Slider
          label={t('adjustments.tint')}
          min={-100}
          max={100}
          step={1}
          defaultValue={0}
          value={colorCalibration.shadowsTint}
          onChange={(e: { target: { value: number | string } }) => handleShadowsChange(String(e.target.value))}
          onDragStateChange={onDragStateChange}
        />
      </div>
      <div className="mt-3">
        <Text color={TextColors.primary} weight={TextWeights.medium} className="mb-3">
          Primaries
        </Text>
        <div className="flex justify-center gap-6 mb-4 px-1">
          {PRIMARY_COLORS.map(({ name, color }) => (
            <ColorSwatch
              color={color}
              isActive={activePrimary === name}
              key={name}
              name={name}
              onClick={setActivePrimary}
            />
          ))}
        </div>
        <Slider
          label={t('adjustments.hue')}
          min={-100}
          max={100}
          step={1}
          defaultValue={0}
          value={currentValues.hue}
          onChange={(e: { target: { value: number | string } }) => handlePrimaryChange('Hue', String(e.target.value))}
          onDragStateChange={onDragStateChange}
        />
        <Slider
          label={t('adjustments.saturation')}
          min={-100}
          max={100}
          step={1}
          defaultValue={0}
          value={currentValues.saturation}
          onChange={(e: { target: { value: number | string } }) =>
            handlePrimaryChange('Saturation', String(e.target.value))
          }
          onDragStateChange={onDragStateChange}
        />
      </div>
    </div>
  );
};

export default function ColorPanel({
  adjustments,
  setAdjustments,
  appSettings,
  isForMask = false,
  isWbPickerActive = false,
  toggleWbPicker,
  onDragStateChange,
}: ColorPanelProps) {
  const { t } = useTranslation();
  const [activeColor, setActiveColor] = useState('reds');
  const adjustmentVisibility = appSettings?.adjustmentVisibility || {};

  const handleGlobalChange = (key: ColorAdjustment, value: string) => {
    setAdjustments((prev: Adjustments) => ({
      ...prev,
      [key]: parseFloat(String(value)),
    }));
  };

  const handleHslChange = (key: ColorAdjustment, value: string) => {
    setAdjustments((prev: Adjustments) => ({
      ...prev,
      hsl: {
        ...(prev.hsl || {}),
        [activeColor]: {
          ...(prev.hsl?.[activeColor] || {}),
          [key]: parseFloat(value),
        },
      },
    }));
  };

  const currentHsl = adjustments?.hsl?.[activeColor] || { hue: 0, saturation: 0, luminance: 0 };

  return (
    <div className="space-y-4">
      <div className="p-2 bg-bg-tertiary rounded-md">
        <div className="flex justify-between items-center mb-2">
          <Text variant={TextVariants.heading}>{t('adjustments.whiteBalance')}</Text>
          {!isForMask && toggleWbPicker && (
            <button
              onClick={toggleWbPicker}
              className={`p-1.5 rounded-md transition-colors ${
                isWbPickerActive ? 'bg-accent text-button-text' : 'hover:bg-bg-secondary text-text-secondary'
              }`}
              data-tooltip={t('adjustments.wbPicker')}
            >
              <Pipette size={16} />
            </button>
          )}
        </div>
        <Slider
          label={t('adjustments.temperature')}
          max={100}
          min={-100}
          onChange={(e: { target: { value: number | string } }) =>
            handleGlobalChange(ColorAdjustment.Temperature, String(e.target.value))
          }
          step={1}
          value={adjustments.temperature || 0}
          onDragStateChange={onDragStateChange}
        />
        <Slider
          label={t('adjustments.tint')}
          max={100}
          min={-100}
          onChange={(e: { target: { value: number | string } }) =>
            handleGlobalChange(ColorAdjustment.Tint, String(e.target.value))
          }
          step={1}
          value={adjustments.tint || 0}
          onDragStateChange={onDragStateChange}
        />
      </div>

      <div className="p-2 bg-bg-tertiary rounded-md">
        <Text variant={TextVariants.heading} className="mb-2">
          {t('adjustments.presence')}
        </Text>
        <Slider
          label={t('adjustments.vibrance')}
          max={100}
          min={-100}
          onChange={(e: { target: { value: number | string } }) =>
            handleGlobalChange(ColorAdjustment.Vibrance, String(e.target.value))
          }
          step={1}
          value={adjustments.vibrance || 0}
          onDragStateChange={onDragStateChange}
        />
        <Slider
          label={t('adjustments.saturation')}
          max={100}
          min={-100}
          onChange={(e: { target: { value: number | string } }) =>
            handleGlobalChange(ColorAdjustment.Saturation, String(e.target.value))
          }
          step={1}
          value={adjustments.saturation || 0}
          onDragStateChange={onDragStateChange}
        />
      </div>

      <div className="p-2 bg-bg-tertiary rounded-md">
        <Text variant={TextVariants.heading} className="mb-3">
          {t('adjustments.colorGrading')}
        </Text>
        <ColorGradingPanel
          adjustments={adjustments}
          setAdjustments={setAdjustments}
          appSettings={appSettings}
          onDragStateChange={onDragStateChange}
        />
      </div>

      <div className="p-2 bg-bg-tertiary rounded-md">
        <Text variant={TextVariants.heading} className="mb-3">
          {t('adjustments.colorMixer')}
        </Text>
        <div className="flex justify-between mb-4 px-1">
          {HSL_COLORS.map(({ name, color }) => (
            <ColorSwatch
              color={color}
              isActive={activeColor === name}
              key={name}
              name={name}
              onClick={setActiveColor}
            />
          ))}
        </div>
        <Slider
          label={t('adjustments.hue')}
          max={100}
          min={-100}
          onChange={(e: { target: { value: number | string } }) =>
            handleHslChange(ColorAdjustment.Hue, String(e.target.value))
          }
          step={1}
          value={currentHsl.hue}
          onDragStateChange={onDragStateChange}
        />
        <Slider
          label={t('adjustments.saturation')}
          max={100}
          min={-100}
          onChange={(e: { target: { value: number | string } }) =>
            handleHslChange(ColorAdjustment.Saturation, String(e.target.value))
          }
          step={1}
          value={currentHsl.saturation}
          onDragStateChange={onDragStateChange}
        />
        <Slider
          label={t('adjustments.luminance')}
          max={100}
          min={-100}
          onChange={(e: { target: { value: number | string } }) =>
            handleHslChange(ColorAdjustment.Luminance, String(e.target.value))
          }
          step={1}
          value={currentHsl.luminance}
          onDragStateChange={onDragStateChange}
        />
      </div>

      {!isForMask && adjustmentVisibility.colorCalibration !== false && (
        <ColorCalibrationPanel
          adjustments={adjustments}
          setAdjustments={setAdjustments}
          appSettings={appSettings}
          onDragStateChange={onDragStateChange}
        />
      )}
    </div>
  );
}
