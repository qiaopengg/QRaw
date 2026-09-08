#!/usr/bin/env python3
"""Reproduce the eDifFIQA observation graph without unsupported BN/Abs ops."""

from __future__ import annotations

import argparse
import hashlib
import shutil
import tempfile
from pathlib import Path

import numpy as np
import onnx
import onnxruntime as ort
from onnx import helper, numpy_helper

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "ediffiqa_tiny_jun2024.onnx"
SOURCE_SHA256 = "9426c899cc0f01665240cb7d9e7f98e18e24e456c178326c771a43da289bfc6a"
OUTPUT = ROOT / "ediffiqa_tiny_jun2024_coreml_qraw.onnx"
OUTPUT_SHA256 = "e25bc043793a0bcda0384f88eb8a04a01d4c6c826ec4a86dc076eba33b863557"


def rewrite(output: Path) -> None:
    if hashlib.sha256(SOURCE.read_bytes()).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("official eDifFIQA model hash mismatch")
    model = onnx.load(SOURCE)
    nodes = {node.name: node for node in model.graph.node}
    tensors = {item.name: numpy_helper.to_array(item) for item in model.graph.initializer}
    bn = nodes["/base_model/bn/BatchNormalization"]
    absolute = nodes["/base_model/Abs"]
    square = nodes["/base_model/Pow"]
    exponent = nodes["/base_model/Constant"]
    attributes = {item.name: helper.get_attribute_value(item) for item in bn.attribute}
    if (bn.op_type != "BatchNormalization" or attributes.get("training_mode") != 0
            or absolute.input[0] != bn.output[0] or square.input[0] != absolute.output[0]
            or square.input[1] != exponent.output[0]
            or float(numpy_helper.to_array(exponent.attribute[0].t)) != 2.0):
        raise RuntimeError("unexpected eDifFIQA normalization graph")
    gamma, beta, mean, variance = (tensors[name] for name in bn.input[1:])
    scale = gamma / np.sqrt(variance + attributes["epsilon"])
    offset = beta - mean * scale
    model.graph.initializer.extend([
        numpy_helper.from_array(scale, "qraw_bn_scale"),
        numpy_helper.from_array(offset, "qraw_bn_offset"),
    ])
    replacements = []
    for node in model.graph.node:
        if node.name == bn.name:
            replacements.extend([
                helper.make_node("Mul", [bn.input[0], "qraw_bn_scale"], ["qraw_bn_scaled"], name="qraw_bn_mul"),
                helper.make_node("Add", ["qraw_bn_scaled", "qraw_bn_offset"], list(bn.output), name="qraw_bn_add"),
            ])
        elif node.name == square.name:
            # abs(x)**2 == x*x; no approximation of the quality regression head.
            replacements.append(helper.make_node("Mul", [bn.output[0], bn.output[0]], list(square.output), name="qraw_square"))
        elif node.name not in (absolute.name, exponent.name):
            replacements.append(node)
    del model.graph.node[:]
    model.graph.node.extend(replacements)
    onnx.checker.check_model(model, full_check=True)
    onnx.save(model, output)


def verify(output: Path) -> tuple[float, list[float]]:
    original = ort.InferenceSession(str(SOURCE), providers=["CPUExecutionProvider"])
    derived = ort.InferenceSession(str(output), providers=["CPUExecutionProvider"])
    rng = np.random.default_rng(20260908)
    gradient = np.empty((1, 3, 112, 112), dtype=np.float32)
    for y in range(112):
        for x in range(112):
            gradient[0, :, y, x] = np.array([x * 2, y * 2, (x + y) % 256], dtype=np.float32) / 127.5 - 1
    inputs = [np.full((1, 3, 112, 112), value, dtype=np.float32) for value in (-1, 0, 1)]
    inputs.append(gradient)
    inputs.extend(rng.uniform(-1, 1, (1, 3, 112, 112)).astype(np.float32) for _ in range(32))
    maximum = 0.0
    reference_scores = []
    for tensor in inputs:
        expected = original.run(["output"], {"input": tensor})[0]
        actual = derived.run(["output"], {"input": tensor})[0]
        if expected.shape != (1, 1) or actual.shape != expected.shape:
            raise RuntimeError("quality output shape mismatch")
        if not np.isfinite(expected).all() or not np.isfinite(actual).all():
            raise RuntimeError("quality output contains non-finite values")
        np.testing.assert_allclose(actual, expected, rtol=1e-5, atol=1e-4)
        maximum = max(maximum, float(np.max(np.abs(actual - expected))))
        reference_scores.append(float(expected[0, 0]))
    return maximum, reference_scores[:4]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true", help="copy verified model only when absent")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="qraw-ediffiqa-") as temporary:
        generated = Path(temporary) / OUTPUT.name
        rewrite(generated)
        maximum, reference_scores = verify(generated)
        digest = hashlib.sha256(generated.read_bytes()).hexdigest()
        if digest != OUTPUT_SHA256:
            raise RuntimeError(f"derived model hash mismatch: {digest}")
        if OUTPUT.exists():
            if hashlib.sha256(OUTPUT.read_bytes()).hexdigest() != digest:
                raise RuntimeError("existing derived model differs; refusing to overwrite")
        elif args.write:
            shutil.copyfile(generated, OUTPUT)
        else:
            raise RuntimeError("derived model missing; rerun with --write")
        print(f"sha256={digest} cases=36 max_abs_error={maximum:.9g}")
        print(f"official CPU scores (black/gray/white/gradient): {reference_scores}")


if __name__ == "__main__":
    main()
