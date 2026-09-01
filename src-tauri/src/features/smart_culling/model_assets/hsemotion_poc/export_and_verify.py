#!/usr/bin/env python3
"""Rewrite and verify the two HSEmotion classifier tails for strict Core ML."""

from __future__ import annotations

import argparse
import hashlib
import shutil
import tempfile
import urllib.request
from pathlib import Path

import numpy as np
import onnx
import onnxruntime as ort
from onnx import TensorProto, helper, numpy_helper


ROOT = Path(__file__).resolve().parent.parent
MODELS = (
    {
        "source_name": "expression_hsemotion_enet_b0_8_va_mtl_qraw_poc.onnx",
        "source_url": "https://raw.githubusercontent.com/sb-ai-lab/EmotiEffLib/main/models/affectnet_emotions/onnx/enet_b0_8_va_mtl.onnx",
        "source_sha256": "c43e056ad388d4a8dc911832b8291435b2af537f967e5870ebd731574ec7e812",
        "output_name": "expression_hsemotion_enet_b0_8_va_mtl_coreml_qraw_poc.onnx",
        "output_sha256": "b11cd798683082eee26c1cc0871aeb5ee545bf7d4330db0b5de3091b00d0eed7",
        "outputs": 10,
    },
    {
        "source_name": "expression_hsemotion_enet_b0_8_best_vgaf_qraw_poc.onnx",
        "source_url": "https://raw.githubusercontent.com/sb-ai-lab/EmotiEffLib/main/models/affectnet_emotions/onnx/enet_b0_8_best_vgaf.onnx",
        "source_sha256": "fa07e841fd06c7a67ee651ea4e6e4a3a2bb5695f47b37a7da50492526f59c898",
        "output_name": "expression_hsemotion_enet_b0_8_best_vgaf_coreml_qraw_poc.onnx",
        "output_sha256": "52383e3d3757286c0ced73ee0aeb50839111b775c8235cac1e43bb6ff16c773e",
        "outputs": 8,
    },
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def rewrite_classifier(source: Path, output: Path, expected_outputs: int) -> None:
    if output.exists():
        raise RuntimeError(f"refusing to overwrite {output}")
    model = onnx.load(source)
    nodes = list(model.graph.node)
    if len(nodes) < 3 or [node.op_type for node in nodes[-3:]] != [
        "GlobalAveragePool",
        "Flatten",
        "Gemm",
    ]:
        raise RuntimeError(f"unexpected HSEmotion classifier tail in {source}")

    global_pool, flatten, gemm = nodes[-3:]
    attributes = {
        attribute.name: helper.get_attribute_value(attribute)
        for attribute in gemm.attribute
    }
    if attributes != {"alpha": 1.0, "beta": 1.0, "transB": 1}:
        raise RuntimeError(f"unsupported Gemm attributes in {source}: {attributes}")

    initializers = {initializer.name: initializer for initializer in model.graph.initializer}
    weights = numpy_helper.to_array(initializers[gemm.input[1]])
    bias = numpy_helper.to_array(initializers[gemm.input[2]])
    if weights.shape != (expected_outputs, 1280) or bias.shape != (expected_outputs,):
        raise RuntimeError(
            f"unexpected classifier parameters in {source}: {weights.shape}, {bias.shape}"
        )

    replacement_weights = numpy_helper.from_array(
        weights[:, :, np.newaxis, np.newaxis], gemm.input[1]
    )
    for index, initializer in enumerate(model.graph.initializer):
        if initializer.name == gemm.input[1]:
            model.graph.initializer[index].CopyFrom(replacement_weights)
            break
    else:
        raise RuntimeError(f"classifier weights are missing in {source}")

    classifier = helper.make_node(
        "Conv",
        inputs=[global_pool.output[0], gemm.input[1], gemm.input[2]],
        outputs=list(gemm.output),
        name=f"{gemm.name}_CoreMLConv",
        dilations=[1, 1],
        group=1,
        kernel_shape=[1, 1],
        pads=[0, 0, 0, 0],
        strides=[1, 1],
    )
    del model.graph.node[-2:]
    model.graph.node.append(classifier)
    model.graph.output[0].type.tensor_type.elem_type = TensorProto.FLOAT
    shape = model.graph.output[0].type.tensor_type.shape
    del shape.dim[:]
    for value in ("batch_size", expected_outputs, 1, 1):
        dimension = shape.dim.add()
        if isinstance(value, str):
            dimension.dim_param = value
        else:
            dimension.dim_value = value

    onnx.checker.check_model(model, full_check=True)
    onnx.save(model, output)


def infer(path: Path, input_tensor: np.ndarray) -> np.ndarray:
    session = ort.InferenceSession(str(path), providers=["CPUExecutionProvider"])
    return session.run(["output"], {"input": input_tensor})[0].reshape(1, -1)


def verify(source: Path, output: Path, expected_outputs: int) -> float:
    rng = np.random.default_rng(20260821)
    inputs = (
        np.zeros((1, 3, 224, 224), dtype=np.float32),
        rng.uniform(-2.2, 2.6, (1, 3, 224, 224)).astype(np.float32),
    )
    maximum_error = 0.0
    for input_tensor in inputs:
        original = infer(source, input_tensor)
        rewritten = infer(output, input_tensor)
        if original.shape != (1, expected_outputs) or rewritten.shape != original.shape:
            raise RuntimeError(
                f"output shape mismatch: original={original.shape}, rewritten={rewritten.shape}"
            )
        maximum_error = max(
            maximum_error, float(np.max(np.abs(original - rewritten)))
        )
    return maximum_error


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--write",
        action="store_true",
        help="copy a verified derived model into model_assets when it is absent",
    )
    args = parser.parse_args()

    with tempfile.TemporaryDirectory(prefix="qraw-hsemotion-") as temporary:
        temporary_dir = Path(temporary)
        for model in MODELS:
            source = ROOT / model["source_name"]
            if not source.exists():
                source = temporary_dir / model["source_name"]
                urllib.request.urlretrieve(model["source_url"], source)
            actual_source_hash = sha256(source)
            if actual_source_hash != model["source_sha256"]:
                raise RuntimeError(
                    f"source hash mismatch for {source}: {actual_source_hash}"
                )

            generated = temporary_dir / model["output_name"]
            rewrite_classifier(source, generated, model["outputs"])
            maximum_error = verify(source, generated, model["outputs"])
            generated_hash = sha256(generated)
            if generated_hash != model["output_sha256"]:
                raise RuntimeError(
                    f"derived hash mismatch for {generated}: {generated_hash}"
                )

            output = ROOT / model["output_name"]
            if output.exists():
                if sha256(output) != generated_hash:
                    raise RuntimeError(f"checked-in model differs from regenerated {output}")
            elif args.write:
                shutil.copyfile(generated, output)
            else:
                raise RuntimeError(f"derived model is missing: {output}; rerun with --write")
            print(
                f"{output.name}: sha256={generated_hash} max_abs_error={maximum_error:.9g}"
            )


if __name__ == "__main__":
    main()
