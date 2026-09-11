"""Fold constant geometry, then express every static ONNX Flatten as Reshape."""

from math import prod

import numpy as np
import onnx
import onnxruntime as ort


def reshape_flattens(model):
    model = onnx.shape_inference.infer_shapes(model, strict_mode=True, data_prop=True)
    values = list(model.graph.input) + list(model.graph.value_info)
    shapes = {value.name: value.type.tensor_type.shape.dim for value in values}
    changed = []
    for node in model.graph.node:
        if node.op_type != "Flatten":
            continue
        dimensions = shapes.get(node.input[0], [])
        if not dimensions or any(not d.HasField("dim_value") or d.dim_value <= 0 for d in dimensions):
            raise ValueError(f"Flatten requires a verified static shape: {node.name}")
        shape = [d.dim_value for d in dimensions]
        axis = next((attribute.i for attribute in node.attribute if attribute.name == "axis"), 1)
        if not -len(shape) <= axis <= len(shape):
            raise ValueError("invalid Flatten axis")
        axis = axis + len(shape) if axis < 0 else axis
        target_shape = [prod(shape[:axis]), prod(shape[axis:])]
        name = node.output[0] + "_fixed_shape"
        if any(value.name == name for value in model.graph.initializer):
            raise ValueError("fixed shape initializer already exists")
        model.graph.initializer.append(onnx.numpy_helper.from_array(np.array(target_shape, dtype=np.int64), name))
        changed.append({"name": node.name, "input_shape": shape, "output_shape": target_shape})
        node.CopyFrom(onnx.helper.make_node("Reshape", [node.input[0], name], list(node.output), name=node.name))
    return model, changed


def prepare_graph(source, output):
    folded = output.parent / "constant_folded.onnx"
    if folded.exists() or output.exists():
        raise FileExistsError("conversion output must be new")
    options = ort.SessionOptions()
    # BASIC folds constants before provider partitioning; no CPU-specific layout/quantization.
    options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_BASIC
    options.optimized_model_filepath = str(folded)
    ort.InferenceSession(str(source), sess_options=options, providers=["CPUExecutionProvider"])
    model, changed = reshape_flattens(onnx.load(folded))
    onnx.save(model, output)
    return {"onnxruntime_version": ort.__version__, "optimization": "ORT_ENABLE_BASIC",
            "flatten_replacements": changed, "node_count": len(model.graph.node)}
