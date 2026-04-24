#!/usr/bin/env python3
"""
Export the analysis-only style-transfer backbone to QRaw's shared model directory.

This script creates:
  - style_transfer_dinov2_vitb.onnx
  - style_transfer_dinov2_vitb.preprocess.json

By default it exports facebook/dinov2-base and writes to ~/.qraw/models/.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import onnx
import torch
from transformers import AutoImageProcessor, AutoModel


DEFAULT_MODEL_ID = "facebook/dinov2-base"
DEFAULT_OUTPUT_NAME = "style_transfer_dinov2_vitb.onnx"
DEFAULT_PREPROCESS_NAME = "style_transfer_dinov2_vitb.preprocess.json"


class StyleTransferBackbone(torch.nn.Module):
    """Return a compact style descriptor that is easy to consume in Rust/ONNX Runtime."""

    def __init__(self, model: torch.nn.Module):
        super().__init__()
        self.model = model

    def forward(self, pixel_values: torch.Tensor) -> torch.Tensor:
        outputs = self.model(pixel_values=pixel_values, return_dict=True)
        tokens = outputs.last_hidden_state
        cls_token = tokens[:, 0, :]
        patch_mean = tokens[:, 1:, :].mean(dim=1)
        return torch.cat([cls_token, patch_mean], dim=1)


def sha256_for(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def resolve_output_size(processor: AutoImageProcessor) -> int:
    crop_size = getattr(processor, "crop_size", None)
    if isinstance(crop_size, dict):
        if "height" in crop_size:
            return int(crop_size["height"])
        if "shortest_edge" in crop_size:
            return int(crop_size["shortest_edge"])
    size = getattr(processor, "size", None)
    if isinstance(size, dict):
        if "shortest_edge" in size:
            return int(size["shortest_edge"])
        if "height" in size:
            return int(size["height"])
    if isinstance(size, int):
        return int(size)
    return 224


def build_preprocess_payload(model_id: str, processor: AutoImageProcessor, image_size: int, hidden_size: int) -> dict:
    return {
        "model_id": model_id,
        "input_name": "pixel_values",
        "output_name": "style_embedding",
        "color_space": "RGB",
        "input_shape": [1, 3, image_size, image_size],
        "do_resize": True,
        "resize_mode": "shortest_edge",
        "do_center_crop": True,
        "crop_size": {
            "height": image_size,
            "width": image_size,
        },
        "size": {
            "shortest_edge": image_size,
        },
        "do_convert_rgb": True,
        "do_rescale": bool(getattr(processor, "do_rescale", True)),
        "rescale_factor": float(getattr(processor, "rescale_factor", 1.0 / 255.0)),
        "do_normalize": bool(getattr(processor, "do_normalize", True)),
        "image_mean": list(getattr(processor, "image_mean", [0.485, 0.456, 0.406])),
        "image_std": list(getattr(processor, "image_std", [0.229, 0.224, 0.225])),
        "interpolation": "bicubic",
        "embedding_strategy": "concat_cls_and_mean_patch_tokens",
        "hidden_size": hidden_size,
        "embedding_size": hidden_size * 2,
    }


def export_backbone(model_id: str, output_dir: Path, force: bool) -> tuple[Path, Path]:
    output_dir.mkdir(parents=True, exist_ok=True)
    onnx_path = output_dir / DEFAULT_OUTPUT_NAME
    preprocess_path = output_dir / DEFAULT_PREPROCESS_NAME

    if onnx_path.exists() and preprocess_path.exists() and not force:
        print(f"[SKIP] {onnx_path.name} and {preprocess_path.name} already exist")
        return onnx_path, preprocess_path

    print(f"[LOAD] model={model_id}")
    processor = AutoImageProcessor.from_pretrained(model_id)
    model = AutoModel.from_pretrained(model_id)
    model.eval()

    image_size = resolve_output_size(processor)
    hidden_size = int(getattr(model.config, "hidden_size", 768))
    wrapper = StyleTransferBackbone(model)

    dummy = torch.randn(1, 3, image_size, image_size, dtype=torch.float32)

    print(f"[EXPORT] {onnx_path}")
    with torch.inference_mode():
        torch.onnx.export(
            wrapper,
            dummy,
            str(onnx_path),
            input_names=["pixel_values"],
            output_names=["style_embedding"],
            dynamic_axes={
                "pixel_values": {0: "batch"},
                "style_embedding": {0: "batch"},
            },
            opset_version=17,
            do_constant_folding=True,
        )

    onnx.checker.check_model(str(onnx_path))

    preprocess_payload = build_preprocess_payload(model_id, processor, image_size, hidden_size)
    preprocess_path.write_text(json.dumps(preprocess_payload, indent=2), encoding="utf-8")

    print(f"[OK] {onnx_path.name}  sha256={sha256_for(onnx_path)}")
    print(f"[OK] {preprocess_path.name}  sha256={sha256_for(preprocess_path)}")
    return onnx_path, preprocess_path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model-id", default=DEFAULT_MODEL_ID)
    parser.add_argument(
        "--output-dir",
        default=str(Path.home() / ".qraw" / "models"),
    )
    parser.add_argument("--force", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    output_dir = Path(args.output_dir).expanduser().resolve()
    export_backbone(args.model_id, output_dir, args.force)


if __name__ == "__main__":
    main()
