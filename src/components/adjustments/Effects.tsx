import Slider from '../ui/Slider';
import { Adjustments, Effect, CreativeAdjustment } from '../../utils/adjustments';
import LUTControl from '../ui/LUTControl';
import { AppSettings } from '../ui/AppProperties';
import Text from '../ui/Text';
import { TextVariants } from '../../types/typography';
import { useTranslation } from 'react-i18next';

type SliderEvent = { target: { value: number | string } };

interface EffectsPanelProps {
  adjustments: Adjustments;
  isForMask: boolean;
  setAdjustments(adjustments: Adjustments | ((prev: Adjustments) => Adjustments)): void;
  handleLutSelect(path: string): void;
  appSettings: AppSettings | null;
  onDragStateChange?: (isDragging: boolean) => void;
}

export default function EffectsPanel({
  adjustments,
  setAdjustments,
  isForMask = false,
  handleLutSelect,
  appSettings,
  onDragStateChange,
}: EffectsPanelProps) {
  const { t } = useTranslation();
  const handleAdjustmentChange = (key: string, value: number | string) => {
    const numericValue = parseInt(String(value), 10);
    setAdjustments((prev: Adjustments) => ({
      ...prev,
      [key]: numericValue,
    }));
  };

  const handleLutIntensityChange = (intensity: number) => {
    setAdjustments((prev: Adjustments) => ({
      ...prev,
      lutIntensity: intensity,
    }));
  };

  const handleLutClear = () => {
    setAdjustments((prev: Adjustments) => ({
      ...prev,
      lutPath: null,
      lutName: null,
      lutData: null,
      lutSize: 0,
      lutIntensity: 100,
    }));
  };

  const adjustmentVisibility = appSettings?.adjustmentVisibility || {};

  return (
    <div className="space-y-4">
      <div className="p-2 bg-bg-tertiary rounded-md">
        <Text variant={TextVariants.heading} className="mb-2">
          {t('adjustments.creative')}
        </Text>

        <Slider
          label={t('adjustments.glow')}
          max={100}
          min={0}
          onChange={(e: SliderEvent) => handleAdjustmentChange(CreativeAdjustment.GlowAmount, e.target.value)}
          step={1}
          value={adjustments.glowAmount}
          onDragStateChange={onDragStateChange}
        />

        <Slider
          label={t('adjustments.halation')}
          max={100}
          min={0}
          onChange={(e: SliderEvent) => handleAdjustmentChange(CreativeAdjustment.HalationAmount, e.target.value)}
          step={1}
          value={adjustments.halationAmount}
          onDragStateChange={onDragStateChange}
        />

        {!isForMask && (
          <Slider
            label={t('adjustments.lightFlares')}
            max={100}
            min={0}
            onChange={(e: SliderEvent) => handleAdjustmentChange(CreativeAdjustment.FlareAmount, e.target.value)}
            step={1}
            value={adjustments.flareAmount}
            onDragStateChange={onDragStateChange}
          />
        )}
      </div>

      {!isForMask && (
        <div className="space-y-4">
          <div className="p-2 bg-bg-tertiary rounded-md">
            <Text variant={TextVariants.heading} className="mb-2">
              {t('adjustments.lut')}
            </Text>
            <LUTControl
              lutName={adjustments.lutName || null}
              lutIntensity={adjustments.lutIntensity || 100}
              onLutSelect={handleLutSelect}
              onIntensityChange={handleLutIntensityChange}
              onClear={handleLutClear}
              onDragStateChange={onDragStateChange}
            />
          </div>

          {adjustmentVisibility.vignette !== false && (
            <div className="p-2 bg-bg-tertiary rounded-md">
              <Text variant={TextVariants.heading} className="mb-2">
                {t('adjustments.vignette')}
              </Text>
              <Slider
                label={t('adjustments.amount')}
                max={100}
                min={-100}
                onChange={(e: SliderEvent) => handleAdjustmentChange(Effect.VignetteAmount, e.target.value)}
                step={1}
                value={adjustments.vignetteAmount}
                onDragStateChange={onDragStateChange}
              />
              <Slider
                defaultValue={50}
                label={t('adjustments.midpoint')}
                max={100}
                min={0}
                onChange={(e: SliderEvent) => handleAdjustmentChange(Effect.VignetteMidpoint, e.target.value)}
                step={1}
                value={adjustments.vignetteMidpoint}
                onDragStateChange={onDragStateChange}
                fillOrigin="min"
              />
              <Slider
                label={t('adjustments.roundness')}
                max={100}
                min={-100}
                onChange={(e: SliderEvent) => handleAdjustmentChange(Effect.VignetteRoundness, e.target.value)}
                step={1}
                value={adjustments.vignetteRoundness}
                onDragStateChange={onDragStateChange}
              />
              <Slider
                defaultValue={50}
                label={t('adjustments.feather')}
                max={100}
                min={0}
                onChange={(e: SliderEvent) => handleAdjustmentChange(Effect.VignetteFeather, e.target.value)}
                step={1}
                value={adjustments.vignetteFeather}
                onDragStateChange={onDragStateChange}
                fillOrigin="min"
              />
            </div>
          )}

          {adjustmentVisibility.grain !== false && (
            <div className="p-2 bg-bg-tertiary rounded-md">
              <Text variant={TextVariants.heading} className="mb-2">
                {t('adjustments.grain')}
              </Text>
              <Slider
                label={t('adjustments.amount')}
                max={100}
                min={0}
                onChange={(e: SliderEvent) => handleAdjustmentChange(Effect.GrainAmount, e.target.value)}
                step={1}
                value={adjustments.grainAmount}
                onDragStateChange={onDragStateChange}
              />
              <Slider
                defaultValue={25}
                label={t('adjustments.size')}
                max={100}
                min={0}
                onChange={(e: SliderEvent) => handleAdjustmentChange(Effect.GrainSize, e.target.value)}
                step={1}
                value={adjustments.grainSize}
                onDragStateChange={onDragStateChange}
                fillOrigin="min"
              />
              <Slider
                defaultValue={50}
                label={t('adjustments.roughness')}
                max={100}
                min={0}
                onChange={(e: SliderEvent) => handleAdjustmentChange(Effect.GrainRoughness, e.target.value)}
                step={1}
                value={adjustments.grainRoughness}
                onDragStateChange={onDragStateChange}
                fillOrigin="min"
              />
            </div>
          )}
        </div>
      )}
    </div>
  );
}
