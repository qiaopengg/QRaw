"""Export the official static network and verify against its frozen CPU baseline."""

import argparse
from contextlib import nullcontext
import hashlib
import json
from pathlib import Path

import onnx
import onnxruntime as ort
import torch

from official import load_model
from export_pooling import fixed_pooling
from export_geometry import fixed_geometry
from export_graph import prepare_graph
from verify_network import baseline_cases, compare_outputs


def run(research, baseline, protocol_path, output, use_fixed_pooling=False):
    output.mkdir(exist_ok=False)
    protocol = json.loads(protocol_path.read_text())
    report = {"scope": "ONNX engineering equivalence only", "cases": [],
              "baseline_sha256": hashlib.sha256((baseline / "REPORT.json").read_bytes()).hexdigest(),
              "protocol_sha256": hashlib.sha256(protocol_path.read_bytes()).hexdigest(),
              "fixed_pooling": use_fixed_pooling,
              "pytorch_parity": [],
              "passed": False, "release_ready": False}
    try:
        torch.set_num_threads(1)
        model, _ = load_model(research)
        _, sample = next(baseline_cases(baseline))
        target = output / "samp_net.onnx"
        traced = output / "traced.onnx" if use_fixed_pooling else target
        tolerance = protocol["onnx_parity"]
        with (torch.inference_mode(),
              fixed_pooling() if use_fixed_pooling else nullcontext(),
              fixed_geometry() if use_fixed_pooling else nullcontext()):
            for row, inputs in baseline_cases(baseline):
                actual = model(*(torch.from_numpy(inputs[k]) for k in ("image", "saliency")))
                values = {key: value.numpy() for key, value in zip(protocol["output_shapes"], actual)}
                errors = compare_outputs(row["outputs"], values, atol=tolerance["absolute_tolerance"],
                                         rtol=tolerance["relative_tolerance"])
                report["pytorch_parity"].append({"id": row["id"], "max_absolute_errors": errors})
            torch.onnx.export(model, tuple(torch.from_numpy(sample[k]) for k in ("image", "saliency")),
                              str(traced), input_names=["image", "saliency"],
                              output_names=list(protocol["output_shapes"]), opset_version=17,
                              dynamo=False, do_constant_folding=True)
        if use_fixed_pooling:
            report["graph_preparation"] = prepare_graph(traced, target)
        onnx.checker.check_model(str(target), full_check=True)
        report["model_sha256"] = hashlib.sha256(target.read_bytes()).hexdigest()
        report["model_bytes"] = target.stat().st_size
        session = ort.InferenceSession(str(target), providers=["CPUExecutionProvider"])
        names = [value.name for value in session.get_outputs()]
        for row, inputs in baseline_cases(baseline):
            values = dict(zip(names, session.run(names, inputs)))
            errors = compare_outputs(row["outputs"], values, atol=tolerance["absolute_tolerance"],
                                     rtol=tolerance["relative_tolerance"])
            report["cases"].append({"id": row["id"], "max_absolute_errors": errors})
        report["passed"] = True
    except Exception as error:
        report["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        with (output / "REPORT.json").open("x") as stream:
            json.dump(report, stream, ensure_ascii=False, indent=2)
            stream.write("\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--research", type=Path, required=True)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--protocol", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--fixed-pooling", action="store_true")
    args = parser.parse_args()
    run(args.research, args.baseline, args.protocol, args.output, args.fixed_pooling)
