"""Compare against the pinned official function without importing its training app."""

import argparse
import ast
import hashlib
import json
from pathlib import Path

import cv2
import numpy as np
from PIL import Image, ImageOps

from preprocess import INPUT_VERSION, prepare_inputs

SOURCE_SHA256 = "24656f1ce9b2c135415e2633ba9570404cfb6cca45f6e892c9bc6c6a9e2753d9"


def official_saliency(source: Path):
    data = source.read_bytes()
    if hashlib.sha256(data).hexdigest() != SOURCE_SHA256:
        raise ValueError("official CADB preprocessing source hash mismatch")
    tree = ast.parse(data)
    functions = [node for node in tree.body if isinstance(node, ast.FunctionDef)
                 and node.name == "detect_saliency"]
    if len(functions) != 1:
        raise ValueError("official saliency entry is missing or duplicated")
    scope = {"cv2": cv2, "np": np}
    # Only this hash-pinned pure function is loaded. No Config/dataset side effects.
    exec(compile(ast.Module(body=functions, type_ignores=[]), str(source), "exec"), scope)
    return scope["detect_saliency"]


def verification_inputs():
    for height, width in [(6, 6), (73, 119), (311, 173), (312, 468)]:
        for value in [0, 128, 255]:
            yield f"constant-{height}x{width}-{value}", np.full((height, width, 3), value, np.uint8)
        y, x = np.indices((height, width))
        yield f"rgb-grid-{height}x{width}", np.stack(
            [(x * 5) % 256, (y * 3) % 256, (x + y) % 256], axis=-1).astype(np.uint8)


def compare(reference, name, rgb):
    before = hashlib.sha256(rgb.tobytes()).hexdigest()
    actual = prepare_inputs(rgb)["saliency"][0, 0]
    expected = reference(rgb.copy()).astype(np.float32)
    np.testing.assert_array_equal(actual, expected)
    if hashlib.sha256(rgb.tobytes()).hexdigest() != before:
        raise AssertionError("preprocessing modified its source pixels")
    return {"input": name, "dimensions": list(rgb.shape[:2][::-1]),
            "max_absolute_error": float(np.max(np.abs(actual - expected)))}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--official-source", required=True, type=Path)
    parser.add_argument("--image", action="append", default=[], type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    # Reserve a new evidence file before running; never overwrite a prior failure.
    with args.output.open("x") as output:
        report = {"input_version": INPUT_VERSION, "official_source_sha256": SOURCE_SHA256,
                  "scope": "preprocessing parity only", "cases": [], "passed": False}
        try:
            reference = official_saliency(args.official_source)
            for name, rgb in verification_inputs():
                report["cases"].append(compare(reference, name, rgb))
            for path in args.image:
                digest = hashlib.sha256(path.read_bytes()).hexdigest()
                with Image.open(path) as image:
                    rgb = np.asarray(ImageOps.exif_transpose(image).convert("RGB"))
                row = compare(reference, str(path), rgb)
                if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
                    raise AssertionError("source image bytes changed")
                report["cases"].append(dict(row, source_sha256=digest))
            report["passed"] = True
        except Exception as error:
            report["error"] = f"{type(error).__name__}: {error}"
            raise
        finally:
            output.write(json.dumps(report, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
