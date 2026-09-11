"""Read-only eye/expression model integrity and ONNX resource isolation audit.

This checks packaging inputs, not an installed application, legal clearance or
recognition accuracy. It cannot enable Release or change runtime capabilities.
"""

import argparse
import hashlib
import json
import re
from pathlib import Path

# Reuse runtime filename/hash constants rather than maintain a second registry.
RUNTIME_CONSTANTS = {
    "face_motion_poc.rs": [
        ("FACE_MESH_MODEL_FILENAME", "FACE_MESH_SHA256"),
        ("FACE_BLENDSHAPES_MODEL_FILENAME", "FACE_BLENDSHAPES_SHA256"),
    ],
    "expression_quality_poc.rs": [
        ("MTL_MODEL_FILENAME", "MTL_MODEL_SHA256"),
        ("VGAF_MODEL_FILENAME", "VGAF_MODEL_SHA256"),
    ],
}


def string_constant(source: str, name: str) -> str:
    values = re.findall(r'const\s+' + re.escape(name) + r'\s*:\s*&str\s*=\s*"([^"\n]+)"\s*;', source)
    if len(values) != 1:
        raise ValueError(f"runtime constant missing or ambiguous: {name}")
    return values[0]


def inspect(feature: Path, resource_dir: Path) -> dict:
    if not resource_dir.is_dir():
        raise ValueError("resource directory is missing; absence is not isolation proof")
    models = []
    for source_name, constants in RUNTIME_CONSTANTS.items():
        source = (feature / source_name).read_text()
        for filename_constant, hash_constant in constants:
            filename = string_constant(source, filename_constant)
            expected = string_constant(source, hash_constant)
            if Path(filename).name != filename or not re.fullmatch(r"[0-9a-f]{64}", expected):
                raise ValueError("invalid runtime model filename/hash contract")
            path = feature / "model_assets" / filename
            actual = hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
            models.append({"file": filename, "runtime_source": source_name,
                           "expected_sha256": expected, "actual_sha256": actual,
                           "matches": actual == expected})
    names = {model["file"] for model in models}
    hashes = {model["expected_sha256"] for model in models}
    packaged = []
    for path in sorted(resource_dir.rglob("*")):
        if path.is_file() and path.suffix.lower() == ".onnx":
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            packaged.append({"path": str(path), "sha256": digest,
                             "calibration_model": path.name in names or digest in hashes})
    # An empty/wrong directory must not yield a reassuring isolation result.
    isolated = bool(packaged) and not any(row["calibration_model"] for row in packaged)
    return {"scope": "eye/expression runtime assets and ONNX packaging inputs only",
            "models": models, "packaged_onnx": packaged,
            "integrity_passed": all(row["matches"] for row in models),
            "packaging_isolated": isolated,
            "release_readiness_assessed": False,
            "remaining": ["weight redistribution clearance", "independent real-camera acceptance",
                          "platform parity and performance", "final installation artifact verification"]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--resource-dir", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    feature = Path(__file__).resolve().parent.parent
    with args.output.open("x") as output:
        try:
            report = inspect(feature, args.resource_dir)
        except Exception as error:
            output.write(json.dumps({"error": f"{type(error).__name__}: {error}"}, indent=2) + "\n")
            raise
        output.write(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    if not report["integrity_passed"] or not report["packaging_isolated"]:
        raise SystemExit("model integrity or resource isolation check failed")


if __name__ == "__main__":
    main()
