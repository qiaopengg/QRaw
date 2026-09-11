"""Official SAMP-Net RGB/spectral-saliency inputs, for offline verification only.

Saliency calculation adapted from BCMI CADB (MIT, see LICENSE-MIT.txt).
No face crop, quality threshold, optical decision or photo rating is produced.
"""

import cv2
import numpy as np
from PIL import Image

INPUT_VERSION = "qraw-sampnet-input-1"
IMAGE_SIZE = 224
MEAN = np.array([0.485, 0.456, 0.406], dtype=np.float32)[:, None, None]
STD = np.array([0.229, 0.224, 0.225], dtype=np.float32)[:, None, None]


def prepare_inputs(rgb: np.ndarray) -> dict[str, np.ndarray]:
    """Accept already oriented uint8 RGB pixels; leave the caller's pixels intact."""
    if (not isinstance(rgb, np.ndarray) or rgb.dtype != np.uint8
            or rgb.ndim != 3 or rgb.shape[2] != 3):
        raise ValueError("SAMP-Net requires uint8 HWC RGB pixels")
    if min(rgb.shape[:2]) < 6:
        raise ValueError("SAMP-Net spectral input requires both dimensions >= 6")
    # Official CADBDataset uses PIL Resize((224,224)), without a center crop.
    resized = Image.fromarray(rgb).resize((IMAGE_SIZE, IMAGE_SIZE), Image.Resampling.BILINEAR)
    image = np.asarray(resized).astype(np.float32).transpose(2, 0, 1) / 255.0
    image = (image - MEAN) / STD
    saliency = _saliency(rgb).astype(np.float32)
    if (not np.isfinite(saliency).all() or saliency.min() < 0
            or saliency.max() > 1):
        raise ValueError("SAMP-Net saliency is not finite in [0,1]")
    return {
        "image": np.ascontiguousarray(image[None]),
        "saliency": np.ascontiguousarray(saliency[None, None]),
    }


def _saliency(rgb: np.ndarray) -> np.ndarray:
    # Preserve upstream order, OpenCV defaults and float64 FFT arithmetic.
    # In particular, derive saliency from the full frame BEFORE the 224px resize.
    gray = cv2.cvtColor(rgb, cv2.COLOR_RGB2GRAY)
    height, width = gray.shape
    reduced = cv2.resize(gray, (width // 6, height // 6), interpolation=cv2.INTER_AREA)
    spectrum = np.fft.fft2(reduced)
    phase = np.angle(spectrum)
    log_amplitude = np.log(np.abs(spectrum) + 0.000001)
    residual = log_amplitude - cv2.blur(log_amplitude, (3, 3))
    reconstructed = np.exp(residual) * (np.cos(phase) + 1j * np.sin(phase))
    saliency = np.abs(np.fft.ifft2(reconstructed)) ** 2
    saliency = cv2.GaussianBlur(saliency, (9, 9), 2.5)
    saliency = cv2.resize(saliency, (IMAGE_SIZE, IMAGE_SIZE), interpolation=cv2.INTER_LINEAR)
    threshold = np.quantile(saliency.reshape(-1), 0.95)
    if threshold > 0:
        saliency[saliency > threshold] = threshold
        # Upstream divides by threshold, not (threshold - minimum).
        saliency = (saliency - saliency.min()) / threshold
    return saliency
