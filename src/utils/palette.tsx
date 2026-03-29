export const generatePaletteFromImage = (imageUrl: string) => {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.crossOrigin = 'Anonymous';
    img.src = imageUrl;

    img.onload = () => {
      const canvas: HTMLCanvasElement = document.createElement('canvas');
      const ctx = canvas.getContext('2d');
      const scale = 0.1;
      const width = img.width * scale;
      const height = img.height * scale;
      canvas.width = width;
      canvas.height = height;
      ctx?.drawImage(img, 0, 0, width, height);

      const imageData = ctx?.getImageData(0, 0, width, height).data;
      if (!imageData) {
        return;
      }

      const sampleRate = 20;
      let bestAccentCandidate = { score: -1, color: { r: 220, g: 220, b: 220 } };

      for (let i = 0; i < imageData.length; i += 4 * sampleRate) {
        const r = imageData[i];
        const g = imageData[i + 1];
        const b = imageData[i + 2];

        const r_ = r / 255,
          g_ = g / 255,
          b_ = b / 255;
        const max = Math.max(r_, g_, b_),
          min = Math.min(r_, g_, b_);
        const l = (max + min) / 2;
        const s = max === min ? 0 : l > 0.5 ? (max - min) / (2 - max - min) : (max - min) / (max + min);

        if (l > 0.3 && l < 0.8 && s > bestAccentCandidate.score) {
          bestAccentCandidate = { score: s, color: { r, g, b } };
        }
      }

      const accentColor = bestAccentCandidate.color;

      const lum = 0.299 * r + 0.587 * g + 0.114 * b;
      if (lum < 140) {
        const t = (1 - lum / 140) * 0.8;
        r += (255 - r) * t;
        g += (255 - g) * t;
        b += (255 - b) * t;
      }

      const mx = Math.max(r, g, b);
      const mn = Math.min(r, g, b);
      const chroma = mx - mn;
      if (chroma > 130) {
        const gray = 0.299 * r + 0.587 * g + 0.114 * b;
        const t = ((chroma - 130) / chroma) * 0.7;
        r += (gray - r) * t;
        g += (gray - g) * t;
        b += (gray - b) * t;
      }

      const accent = `rgb(${Math.round(r)}, ${Math.round(g)}, ${Math.round(b)})`;

      resolve({
        '--app-accent': accent,
        '--app-hover-color': accent,
      });
    };

    img.onerror = (err) => {
      console.error('Failed to load image for palette generation:', err);
      reject(err);
    };
  });
};
