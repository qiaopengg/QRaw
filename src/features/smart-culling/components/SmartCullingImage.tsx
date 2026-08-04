import { useState, type ImgHTMLAttributes, type ReactNode } from 'react';

interface SmartCullingImageProps extends Omit<ImgHTMLAttributes<HTMLImageElement>, 'src'> {
  primaryUrl?: string | null;
  fallbackUrl?: string | null;
  fallback: ReactNode;
}

export function SmartCullingImage({
  primaryUrl,
  fallbackUrl,
  fallback,
  onError,
  ...imageProps
}: SmartCullingImageProps) {
  const [failedUrls, setFailedUrls] = useState<ReadonlySet<string>>(() => new Set());
  const sources = [primaryUrl, fallbackUrl].filter(
    (source, index, values): source is string => Boolean(source) && values.indexOf(source) === index,
  );
  const activeUrl = sources.find((source) => !failedUrls.has(source));

  if (!activeUrl) return fallback;
  return (
    <img
      {...imageProps}
      src={activeUrl}
      onError={(event) => {
        setFailedUrls((current) => new Set(current).add(activeUrl));
        onError?.(event);
      }}
    />
  );
}
