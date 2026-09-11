"""Read frozen baseline tensors and compare all three network outputs."""

import hashlib
import json
from pathlib import Path

import numpy as np

from official import WEIGHT_SHA256


def baseline_cases(directory: Path):
    report = json.loads((directory / "REPORT.json").read_text())
    if not report["passed"] or report["weight_sha256"] != WEIGHT_SHA256 or len(report["cases"]) != 19:
        raise ValueError("complete official baseline is required")
    for row in report["cases"]:
        inputs = {}
        for name, shape in [("image", [1, 3, 224, 224]), ("saliency", [1, 1, 224, 224])]:
            spec = row["inputs"][name]
            if Path(spec["file"]).name != spec["file"] or spec["shape"] != shape:
                raise ValueError("invalid baseline input path or shape")
            data = (directory / spec["file"]).read_bytes()
            if hashlib.sha256(data).hexdigest() != spec["sha256"]:
                raise ValueError("baseline input hash mismatch")
            inputs[name] = np.frombuffer(data, dtype="<f4").reshape(shape).copy()
        yield row, inputs


def compare_outputs(expected, actual, *, atol, rtol):
    errors = {}
    if set(actual) != set(expected):
        raise ValueError("all baseline outputs must be compared")
    for name in expected:
        reference = np.asarray(expected[name], dtype=np.float32)
        value = np.asarray(actual[name])
        if value.shape != reference.shape or not np.isfinite(value).all():
            raise ValueError(f"invalid output: {name}")
        np.testing.assert_allclose(value, reference, atol=atol, rtol=rtol)
        errors[name] = float(np.max(np.abs(value - reference)))
    return errors
