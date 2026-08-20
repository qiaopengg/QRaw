#!/usr/bin/env python3
"""Export and verify an isolated ARNIQA ONNX proof of concept.

This script deliberately produces two graphs:

* a shared, fixed-shape ResNet-50 encoder that returns the raw 2048-D quality
  features for one 224 x 224 crop; and
* a very small, replaceable three-regressor head for SPAQ, KonIQ-10k and
  KADID-10k.

Image resizing, five-crop extraction, ImageNet normalization, L2 feature
normalization, half-scale generation, feature concatenation and crop
aggregation remain outside the graph. Keeping the two cheap feature operations
outside ONNX avoids known Core ML EP partition gaps while preserving the
upstream numeric contract. This is research tooling only. It is not wired into
QRaw's production model loader or scoring path.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import sys
from collections import Counter
from pathlib import Path
from typing import Dict, Mapping, Tuple

import numpy as np
import onnx
import onnxruntime as ort
import torch
import torch.nn as nn
import torch.nn.functional as functional
from torchvision.models import resnet50


UPSTREAM_ENCODER_SHA256 = "ad2022e59b1040d5bab24f9325c10d0215956a2061248a36c15edaec3e60fcd1"
UPSTREAM_RESNET_SOURCE_SHA256 = (
    "07084106f7e096529fc584d755a5ab9f9ef94fdaf575a85053b9b39604140c49"
)
UPSTREAM_HEAD_SHA256 = {
    "spaq": "dbee93f9a8deb3c8357af0b7d4598c153b4a10075be34f9b69daa1aa04e778e3",
    "koniq10k": "af8f127aca38a8e1082e066b5ee93e533bf5f33aaf9c76b3c31526ef901919e1",
    "kadid10k": "4315bf471d52eb7d3e5de1e2ac8bb465f8eec10cd724103c72a178b9aaa4aa3f",
}
DATASETS = ("spaq", "koniq10k", "kadid10k")
DATASET_RANGES = {
    "spaq": (1.0, 100.0),
    "koniq10k": (1.0, 100.0),
    "kadid10k": (1.0, 5.0),
}


class FixedArniqaEncoder(nn.Module):
    """Upstream ResNet-50 trunk with the unused projection head removed."""

    def __init__(self) -> None:
        super().__init__()
        model = resnet50(weights=None)
        self.backbone = nn.Sequential(*list(model.children())[:-1])

    def forward(self, normalized_rgb: torch.Tensor) -> torch.Tensor:
        return self.backbone(normalized_rgb).flatten(1)


class ArniqaThreeHeads(nn.Module):
    """Three official ridge heads sharing the same pair of embeddings."""

    def __init__(self, weights: torch.Tensor, biases: torch.Tensor) -> None:
        super().__init__()
        self.register_buffer("weights", weights)
        self.register_buffer("biases", biases)
        minima = torch.tensor([DATASET_RANGES[name][0] for name in DATASETS])
        widths = torch.tensor(
            [DATASET_RANGES[name][1] - DATASET_RANGES[name][0] for name in DATASETS]
        )
        self.register_buffer("minima", minima)
        self.register_buffer("widths", widths)

    def forward(
        self, combined_embedding: torch.Tensor
    ) -> Tuple[torch.Tensor, torch.Tensor]:
        raw_scores = functional.linear(combined_embedding, self.weights, self.biases)
        scaled_scores = (raw_scores - self.minima) / self.widths
        return raw_scores, scaled_scores


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def require_hash(path: Path, expected: str) -> None:
    actual = sha256(path)
    if actual != expected:
        raise RuntimeError(f"SHA-256 mismatch for {path}: expected {expected}, got {actual}")


def load_upstream_resnet(official_repo: Path, checkpoint: Path) -> nn.Module:
    module_path = official_repo / "models" / "resnet.py"
    if not module_path.is_file():
        raise FileNotFoundError(f"official models/resnet.py not found under {official_repo}")
    require_hash(module_path, UPSTREAM_RESNET_SOURCE_SHA256)

    spec = importlib.util.spec_from_file_location("official_arniqa_resnet", module_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import official ARNIQA source: {module_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)

    official = module.ResNet(embedding_dim=128, pretrained=False, use_norm=True).eval()
    state = torch.load(checkpoint, map_location="cpu", weights_only=True)
    official.load_state_dict(state, strict=True)
    return official


def load_export_encoder(checkpoint: Path) -> FixedArniqaEncoder:
    state = torch.load(checkpoint, map_location="cpu", weights_only=True)
    backbone_state = {
        key.removeprefix("model."): value
        for key, value in state.items()
        if key.startswith("model.")
    }
    encoder = FixedArniqaEncoder().eval()
    encoder.backbone.load_state_dict(backbone_state, strict=True)
    return encoder


def load_heads(weights_dir: Path) -> Tuple[ArniqaThreeHeads, Mapping[str, nn.Module]]:
    upstream_heads: Dict[str, nn.Module] = {}
    weights = []
    biases = []
    for dataset in DATASETS:
        path = weights_dir / f"regressor_{dataset}.pth"
        require_hash(path, UPSTREAM_HEAD_SHA256[dataset])
        head = torch.jit.load(str(path), map_location="cpu").eval()
        state = head.state_dict()
        weights.append(state["weights"].reshape(1, 4096))
        biases.append(state["biases"].reshape(1))
        upstream_heads[dataset] = head
    model = ArniqaThreeHeads(torch.cat(weights, dim=0), torch.cat(biases, dim=0)).eval()
    return model, upstream_heads


def deterministic_inputs() -> Tuple[torch.Tensor, torch.Tensor]:
    generator = torch.Generator(device="cpu").manual_seed(20260820)
    full = torch.randn((1, 3, 224, 224), generator=generator, dtype=torch.float32)
    half = torch.randn((1, 3, 224, 224), generator=generator, dtype=torch.float32)
    return full, half


def max_abs(left: np.ndarray, right: np.ndarray) -> float:
    return float(np.max(np.abs(left.astype(np.float64) - right.astype(np.float64))))


def export_graphs(
    encoder: FixedArniqaEncoder,
    heads: ArniqaThreeHeads,
    output_dir: Path,
    full: torch.Tensor,
    half: torch.Tensor,
) -> Tuple[Path, Path]:
    output_dir.mkdir(parents=True, exist_ok=True)
    encoder_path = output_dir / "arniqa_encoder_224_poc.onnx"
    heads_path = output_dir / "arniqa_three_heads_poc.onnx"

    torch.onnx.export(
        encoder,
        (full,),
        str(encoder_path),
        input_names=["normalized_rgb"],
        output_names=["features"],
        opset_version=17,
        do_constant_folding=True,
        dynamo=False,
    )
    with torch.no_grad():
        embedding_full = functional.normalize(encoder(full), dim=1)
        embedding_half = functional.normalize(encoder(half), dim=1)
        combined_embedding = torch.cat((embedding_full, embedding_half), dim=1)
    torch.onnx.export(
        heads,
        (combined_embedding,),
        str(heads_path),
        input_names=["combined_embedding"],
        output_names=["raw_scores", "scaled_scores"],
        opset_version=17,
        do_constant_folding=True,
        dynamo=False,
    )
    return encoder_path, heads_path


def graph_summary(path: Path) -> Mapping[str, object]:
    model = onnx.load(str(path))
    onnx.checker.check_model(model, full_check=True)
    operators = Counter(node.op_type for node in model.graph.node)
    return {
        "bytes": path.stat().st_size,
        "sha256": sha256(path),
        "opset": [(item.domain or "ai.onnx", item.version) for item in model.opset_import],
        "inputs": [
            {
                "name": item.name,
                "shape": [dim.dim_value for dim in item.type.tensor_type.shape.dim],
            }
            for item in model.graph.input
        ],
        "outputs": [
            {
                "name": item.name,
                "shape": [dim.dim_value for dim in item.type.tensor_type.shape.dim],
            }
            for item in model.graph.output
        ],
        "operators": dict(sorted(operators.items())),
    }


def cpu_onnx_alignment(
    encoder_path: Path,
    heads_path: Path,
    torch_embeddings: Tuple[torch.Tensor, torch.Tensor],
    torch_scores: Tuple[torch.Tensor, torch.Tensor],
    inputs: Tuple[torch.Tensor, torch.Tensor],
) -> Mapping[str, float]:
    encoder_session = ort.InferenceSession(str(encoder_path), providers=["CPUExecutionProvider"])
    heads_session = ort.InferenceSession(str(heads_path), providers=["CPUExecutionProvider"])
    full, half = inputs
    onnx_full_raw = encoder_session.run(None, {"normalized_rgb": full.numpy()})[0]
    onnx_half_raw = encoder_session.run(None, {"normalized_rgb": half.numpy()})[0]
    onnx_full = onnx_full_raw / np.linalg.norm(onnx_full_raw, axis=1, keepdims=True)
    onnx_half = onnx_half_raw / np.linalg.norm(onnx_half_raw, axis=1, keepdims=True)
    raw, scaled = heads_session.run(
        None,
        {"combined_embedding": np.concatenate((onnx_full, onnx_half), axis=1)},
    )
    torch_full, torch_half = (item.detach().numpy() for item in torch_embeddings)
    torch_raw, torch_scaled = (item.detach().numpy() for item in torch_scores)
    return {
        "encoder_full_max_abs": max_abs(onnx_full, torch_full),
        "encoder_half_max_abs": max_abs(onnx_half, torch_half),
        "raw_scores_max_abs": max_abs(raw, torch_raw),
        "scaled_scores_max_abs": max_abs(scaled, torch_scaled),
    }


def official_alignment(
    official: nn.Module,
    encoder: FixedArniqaEncoder,
    heads: ArniqaThreeHeads,
    upstream_heads: Mapping[str, nn.Module],
    inputs: Tuple[torch.Tensor, torch.Tensor],
) -> Tuple[
    Mapping[str, float],
    Tuple[torch.Tensor, torch.Tensor],
    Tuple[torch.Tensor, torch.Tensor],
]:
    full, half = inputs
    with torch.inference_mode():
        official_full = official(full)[0]
        official_half = official(half)[0]
        export_full = functional.normalize(encoder(full), dim=1)
        export_half = functional.normalize(encoder(half), dim=1)
        raw, scaled = heads(torch.cat((export_full, export_half), dim=1))
        combined = torch.cat((official_full, official_half), dim=1)
        upstream_raw = torch.stack(
            [upstream_heads[name](combined).reshape(-1) for name in DATASETS], dim=1
        )
        minima = torch.tensor([DATASET_RANGES[name][0] for name in DATASETS])
        widths = torch.tensor(
            [DATASET_RANGES[name][1] - DATASET_RANGES[name][0] for name in DATASETS]
        )
        upstream_scaled = (upstream_raw - minima) / widths

    result = {
        "encoder_full_max_abs": max_abs(export_full.numpy(), official_full.numpy()),
        "encoder_half_max_abs": max_abs(export_half.numpy(), official_half.numpy()),
        "raw_scores_max_abs": max_abs(raw.numpy(), upstream_raw.numpy()),
        "scaled_scores_max_abs": max_abs(scaled.numpy(), upstream_scaled.numpy()),
    }
    return result, (export_full, export_half), (raw, scaled)


def ensure_thresholds(alignment: Mapping[str, float], thresholds: Mapping[str, float]) -> None:
    failures = [
        f"{name}={value} exceeds {thresholds[name]}"
        for name, value in alignment.items()
        if value > thresholds[name]
    ]
    if failures:
        raise RuntimeError("numeric alignment failed: " + "; ".join(failures))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--official-repo", type=Path, required=True)
    parser.add_argument("--weights-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    checkpoint = args.weights_dir / "ARNIQA.pth"
    require_hash(checkpoint, UPSTREAM_ENCODER_SHA256)
    official = load_upstream_resnet(args.official_repo, checkpoint)
    encoder = load_export_encoder(checkpoint)
    heads, upstream_heads = load_heads(args.weights_dir)
    inputs = deterministic_inputs()

    upstream_alignment, embeddings, scores = official_alignment(
        official, encoder, heads, upstream_heads, inputs
    )
    ensure_thresholds(
        upstream_alignment,
        {
            "encoder_full_max_abs": 1e-7,
            "encoder_half_max_abs": 1e-7,
            "raw_scores_max_abs": 1e-5,
            "scaled_scores_max_abs": 1e-7,
        },
    )

    encoder_path, heads_path = export_graphs(encoder, heads, args.output_dir, *inputs)
    onnx_alignment = cpu_onnx_alignment(
        encoder_path, heads_path, embeddings, scores, inputs
    )
    ensure_thresholds(
        onnx_alignment,
        {
            "encoder_full_max_abs": 1e-5,
            "encoder_half_max_abs": 1e-5,
            "raw_scores_max_abs": 5e-3,
            "scaled_scores_max_abs": 5e-5,
        },
    )

    report = {
        "versions": {
            "python": sys.version.split()[0],
            "torch": torch.__version__,
            "onnx": onnx.__version__,
            "onnxruntime": ort.__version__,
            "providers": ort.get_available_providers(),
        },
        "upstream_alignment": upstream_alignment,
        "onnx_cpu_alignment": onnx_alignment,
        "graphs": {
            encoder_path.name: graph_summary(encoder_path),
            heads_path.name: graph_summary(heads_path),
        },
        "sample_raw_scores": scores[0].detach().numpy().tolist(),
        "sample_scaled_scores": scores[1].detach().numpy().tolist(),
    }
    print(json.dumps(report, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
