#!/bin/bash

set -euo pipefail

MODELS_DIR="${QRAW_MODELS_DIR:-$HOME/.qraw/models}"
mkdir -p "$MODELS_DIR"

download_and_verify() {
  local filename="$1"
  local url="$2"
  local expected_sha="$3"
  local dest="$MODELS_DIR/$filename"
  local tmp="$dest.part"

  echo "==> $filename"

  if [ -f "$dest" ]; then
    local actual_sha
    actual_sha="$(shasum -a 256 "$dest" | awk '{print $1}')"
    if [ "$actual_sha" = "$expected_sha" ]; then
      echo "    already verified"
      return 0
    fi
    echo "    existing file hash mismatch, redownloading"
    rm -f "$dest"
  fi

  rm -f "$tmp"
  curl -L --fail --retry 3 --retry-delay 2 --connect-timeout 20 -o "$tmp" "$url"

  local actual_sha
  actual_sha="$(shasum -a 256 "$tmp" | awk '{print $1}')"
  if [ "$actual_sha" != "$expected_sha" ]; then
    echo "    sha256 mismatch"
    echo "    expected: $expected_sha"
    echo "    actual:   $actual_sha"
    rm -f "$tmp"
    return 1
  fi

  mv "$tmp" "$dest"
  stat -f "    saved: %N %z bytes" "$dest"
}

download_and_verify \
  "sam_vit_b_01ec64_encoder.onnx" \
  "https://hf-mirror.com/CyberTimon/RapidRAW-Models/resolve/main/sam_vit_b_01ec64_encoder.onnx?download=true" \
  "16ab73d9c824886f0de2938c19df22fb9ec3deebfd0de58e65177e479213d7d1"

download_and_verify \
  "sam_vit_b_01ec64_decoder.onnx" \
  "https://hf-mirror.com/CyberTimon/RapidRAW-Models/resolve/main/sam_vit_b_01ec64_decoder.onnx?download=true" \
  "85d0d672cf5b7fe763edcde429e5533e62f674af4b15c7d688b7673b0ef00bf7"

download_and_verify \
  "u2net.onnx" \
  "https://hf-mirror.com/CyberTimon/RapidRAW-Models/resolve/main/u2net.onnx?download=true" \
  "8d10d2f3bb75ae3b6d527c77944fc5e7dcd94b29809d47a739a7a728a912b491"

download_and_verify \
  "skyseg_u2net.onnx" \
  "https://hf-mirror.com/CyberTimon/RapidRAW-Models/resolve/main/skyseg-u2net.onnx?download=true" \
  "ab9c34c64c3d821220a2886a4a06da4642ffa14d5b30e8d5339056a089aa1d39"

download_and_verify \
  "depth_anything_v2_vits.onnx" \
  "https://hf-mirror.com/CyberTimon/RapidRAW-Models/resolve/main/depth_anything_v2_vits.onnx?download=true" \
  "d2b11a11c1d4a12b47608fa65a17ee9a4c605b55ee1730c8e3b526304f2562be"

download_and_verify \
  "style_transfer_dinov2_vitb.onnx" \
  "https://huggingface.co/onnx-community/dinov2-base-ONNX/resolve/main/onnx/model.onnx?download=true" \
  "f16115e628d65b7cc7b1e16c504e2af682169aabf3fff4edfe906118f522e204"

download_and_verify \
  "style_transfer_dinov2_vitb.preprocess.json" \
  "https://huggingface.co/onnx-community/dinov2-base-ONNX/resolve/main/preprocessor_config.json?download=true" \
  "14e780d86fa1861f8751f868d7f45425b5feb55c38ca26f152ca5097ab30f828"

echo "Done."
