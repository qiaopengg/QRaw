import { useCallback, useEffect } from 'react';

export const useKeydown = (key: string, callback: () => void, enabled = true) => {
  const memoizedCallback = useCallback(callback, [callback]);

  useEffect(() => {
    if (!enabled) {
      return;
    }

    const handler = (e: KeyboardEvent) => {
      if (e.key.toLowerCase() === key.toLowerCase()) {
        if (document.activeElement?.tagName?.toLowerCase() === 'input') {
          return;
        }
        e.preventDefault();
        memoizedCallback();
      }
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [key, memoizedCallback, enabled]);
};
