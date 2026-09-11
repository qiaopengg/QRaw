"""Run official weights and freeze exact inputs/outputs for downstream parity."""

import argparse
import hashlib
import json
import time
from pathlib import Path

import numpy as np
from PIL import Image, ImageOps
import torch

from official import WEIGHT_SHA256, input_reference, load_model
from preprocess import INPUT_VERSION, prepare_inputs
from verify_preprocess import verification_inputs


def check_outputs(outputs):
    names = ("pattern_logits", "attributes", "composition_distribution")
    result = {}
    for name, output, shape in zip(names, outputs, [(1, 8), (1, 6), (1, 5)]):
        value = output.detach().cpu().numpy()
        if value.shape != shape or not np.isfinite(value).all():
            raise ValueError(f"invalid {name}: {value.shape}")
        result[name] = value
    distribution = result["composition_distribution"]
    if np.any(distribution < 0) or np.any(distribution > 1):
        raise ValueError("composition distribution outside [0,1]")
    np.testing.assert_allclose(distribution.sum(1), 1, rtol=0, atol=1e-6)
    return result


def cases(protocol):
    yield from verification_inputs()
    for item in protocol["images"]:
        path = Path(item["copy"])
        if hashlib.sha256(path.read_bytes()).hexdigest() != item["sha256"]:
            raise ValueError("desktop input hash mismatch")
        with Image.open(path) as image:
            yield str(path), np.asarray(ImageOps.exif_transpose(image).convert("RGB"))


def run(research, protocol_path, output):
    output.mkdir(exist_ok=False)
    protocol = json.loads(protocol_path.read_text())
    report = {"scope": "official network baseline; not optical/composition acceptance",
              "input_version": INPUT_VERSION, "weight_sha256": WEIGHT_SHA256,
              "protocol_sha256": hashlib.sha256(protocol_path.read_bytes()).hexdigest(),
              "cases": [], "passed": False, "release_ready": False}
    try:
        torch.set_num_threads(1)
        model, cfg = load_model(research)
        transform, saliency = input_reference(research)
        report["attributes"] = cfg.attribute_types
        report["strict_state_tensor_count"] = len(model.state_dict())
        for index, (name, rgb) in enumerate(cases(protocol)):
            pixels_hash = hashlib.sha256(rgb.tobytes()).hexdigest()
            inputs = prepare_inputs(rgb)
            official = {"image": transform(Image.fromarray(rgb)).numpy()[None],
                        "saliency": saliency(rgb.copy()).astype(np.float32)[None, None]}
            for key in inputs:
                np.testing.assert_array_equal(inputs[key], official[key])
            started = time.perf_counter()
            with torch.inference_mode():
                outputs = check_outputs(model(*(torch.from_numpy(inputs[k]) for k in ("image", "saliency"))))
            row = {"id": f"case-{index:02d}", "input": name, "inputs": {},
                   "outputs": {key: value.tolist() for key, value in outputs.items()},
                   "inference_seconds": time.perf_counter() - started}
            for key, value in inputs.items():
                filename = f"{row['id']}-{key}.f32le"
                data = value.astype("<f4").tobytes()
                with (output / filename).open("xb") as stream:
                    stream.write(data)
                row["inputs"][key] = {"file": filename, "shape": list(value.shape),
                                      "sha256": hashlib.sha256(data).hexdigest()}
            if hashlib.sha256(rgb.tobytes()).hexdigest() != pixels_hash:
                raise ValueError("source pixels changed")
            report["cases"].append(row)
        if len(report["cases"]) != 19:
            raise ValueError("baseline must contain all 19 predeclared cases")
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
    parser.add_argument("--protocol", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    run(args.research, args.protocol, args.output)
