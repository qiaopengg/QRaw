#!/bin/bash
# QRaw AI Culling V4 — Model Download Script
# Downloads all required ONNX models to ~/.qraw/models/
# No Python/PyTorch/TensorFlow needed — all models are pre-built ONNX files.

set -e

MODELS_DIR="$HOME/.qraw/models"
mkdir -p "$MODELS_DIR"

echo "============================================================"
echo "QRaw AI Culling V4 — Model Download"
echo "Target: $MODELS_DIR"
echo "============================================================"

download() {
    local url="$1"
    local dest="$2"
    local name="$3"

    if [ -f "$dest" ]; then
        local size=$(stat -f%z "$dest" 2>/dev/null || stat -c%s "$dest" 2>/dev/null)
        if [ "$size" -gt 10000 ]; then
            echo "  ✅ $name already exists ($(echo "scale=1; $size/1048576" | bc) MB)"
            return 0
        fi
    fi

    echo "  ⬇️  Downloading $name ..."
    if curl -L -sS --fail -o "$dest" "$url"; then
        local size=$(stat -f%z "$dest" 2>/dev/null || stat -c%s "$dest" 2>/dev/null)
        echo "  ✅ $name ($(echo "scale=1; $size/1048576" | bc) MB)"
    else
        echo "  ❌ Failed to download $name"
        rm -f "$dest"
        return 1
    fi
}

echo ""
echo "── Required Models ──"

# YOLOv8n-Face (face detection, required)
download \
    "https://mirror.ghproxy.com/https://github.com/akanametov/yolov8-face/releases/download/v0.0.0/yolov8n-face.onnx" \
    "$MODELS_DIR/yolov8n-face.onnx" \
    "YOLOv8n-Face (face detection)"

# YuNet (face detection, preferred)
download \
    "https://mirror.ghproxy.com/https://github.com/opencv/opencv_zoo/raw/main/models/face_detection_yunet/face_detection_yunet_2023mar.onnx" \
    "$MODELS_DIR/face_detection_yunet_2023mar.onnx" \
    "YuNet (face detection)"

# FerPlus (expression classification, fallback)
download \
    "https://mirror.ghproxy.com/https://github.com/onnx/models/raw/main/validated/vision/body_analysis/emotion_ferplus/model/emotion-ferplus-8.onnx" \
    "$MODELS_DIR/emotion-ferplus-8.onnx" \
    "FerPlus (expression)"

echo ""
echo "── Enhancement Models ──"

# InsightFace 2d106det (106-point landmark, enables EAR blink detection)
download \
    "https://hf-mirror.com/fofr/comfyui/resolve/main/insightface/models/buffalo_l/2d106det.onnx" \
    "$MODELS_DIR/2d106det.onnx" \
    "InsightFace 2d106det (blink detection)"

# HSEmotion ONNX (better expression model, replaces FerPlus)
# From hsemotion-onnx package — the model is hosted on GitHub
download \
    "https://mirror.ghproxy.com/https://github.com/HSE-asavchenko/hsemotion-onnx/raw/main/hsemotion_onnx/models/enet_b0_8_best_afew.onnx" \
    "$MODELS_DIR/hsemotion.onnx" \
    "HSEmotion (expression, AffectNet-trained)"

# NIMA Aesthetic — try from idealo's pre-exported ONNX on HuggingFace
# Note: If this URL doesn't work, NIMA aesthetic will be skipped (optional)
download \
    "https://hf-mirror.com/chavinlo/nima-aesthetic/resolve/main/nima_aesthetic.onnx" \
    "$MODELS_DIR/nima.onnx" \
    "NIMA Aesthetic (image quality)" || true

echo ""
echo "============================================================"
echo "Final Status:"
echo "============================================================"
for f in yolov8n-face.onnx face_detection_yunet_2023mar.onnx emotion-ferplus-8.onnx 2d106det.onnx hsemotion.onnx nima.onnx nima_technical.onnx; do
    if [ -f "$MODELS_DIR/$f" ]; then
        size=$(stat -f%z "$MODELS_DIR/$f" 2>/dev/null || stat -c%s "$MODELS_DIR/$f" 2>/dev/null)
        echo "  ✅ $f ($(echo "scale=1; $size/1048576" | bc) MB)"
    else
        echo "  ❌ $f (missing — optional)"
    fi
done
echo ""
echo "Done! Core models are ready. Missing optional models won't affect basic functionality."
