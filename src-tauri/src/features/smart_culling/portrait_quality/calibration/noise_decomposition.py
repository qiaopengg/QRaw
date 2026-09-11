"""Versioned diagnosis of every paired degradation; parent failures remain read-only."""

import argparse
import json
from pathlib import Path

import paired_degradation as paired
from decomposition_checks import TOLERANCE, verify_numerics
from legacy import content_hash, utc_now
from prepare import write_new
from stress import FEATURE

FILES = list(dict.fromkeys([*paired.FILES, *[f"portrait_quality/{name}.rs" for name in (
    "mechanism_energy", "mechanism_samples", "degradation_intermediates", "decomposition_trace", "decomposition_probe",
    "noise_covariance", "noise_covariance_tests", "noise_decomposition", "noise_decomposition_tests")],
    *[f"portrait_quality/calibration/{name}.py" for name in (
        "noise_decomposition", "decomposition_checks", "decomposition_report", "test_noise_decomposition", "candidate", "prepare", "stress")]]))
PARENT_FILES = ("PROTOCOL.json", "DESIGN.md", "observations.json", "SUMMARY.json", "FAILURE_CASES.json",
                "FAILURE_ANALYSIS.md", "VALIDATION.json", "FINAL_RECHECK.json", "CODE_MAP.json")
INHERITED = ("frozen_path", "data_version", "sample_count", "samples", "frozen_file_hashes", "cases")


def parent_read(parent, *, reader=paired.checked_read):
    root = parent / "measurement_code" / FEATURE.relative_to(FEATURE.parents[3])
    protocol, rows, *_ = reader(parent, code_root=root)
    mapping = json.loads((parent / "CODE_MAP.json").read_text())
    if set(mapping) != set(protocol["code_hashes"]):
        raise ValueError("父实验代码归档清单不完整")
    for name, digest in protocol["code_hashes"].items():
        if (mapping[name]["sha256"] != digest or (parent / mapping[name]["path"]).resolve() != (root / name).resolve()):
            raise ValueError("父实验归档映射错误")
    return protocol, rows, root


def prepare(parent, output):
    previous, _, root = parent_read(parent)
    protocol = dict(schema="qraw-noise-decomposition-protocol-1", created_at=utc_now(),
        **{key: previous[key] for key in INHERITED}, parent_path=str(parent), parent_code_root=str(root),
        parent_file_hashes={name: content_hash(parent / name) for name in PARENT_FILES},
        parent_code_hashes=previous["code_hashes"], code_hashes={name: content_hash(FEATURE / name) for name in FILES},
        design_sha256=content_hash(output / "DESIGN.md"), identity_tolerance=TOLERANCE,
        source_provenance=previous["source_provenance"], reference_used_for_diagnosis_only=True,
        human_blur_labels_created=False, candidate_fitted=False, runtime_enabled=False, release_ready=False)
    write_new(output / "PROTOCOL.json", protocol)
    print(f"已固定 {protocol['sample_count']}×{len(protocol['cases'])} 条件；尚未分解")
    return protocol


def checked_read(output, *, code_root=FEATURE):
    protocol = json.loads((output / "PROTOCOL.json").read_text())
    observed = json.loads((output / "observations.json").read_text())
    if (protocol["schema"] != "qraw-noise-decomposition-protocol-1"
            or observed["schema"] != "qraw-noise-decomposition-observations-1"
            or protocol["identity_tolerance"] != TOLERANCE or set(protocol["code_hashes"]) != set(FILES)
            or protocol["design_sha256"] != content_hash(output / "DESIGN.md")
            or observed["protocol_sha256"] != content_hash(output / "PROTOCOL.json")
            or observed["code_hashes"] != protocol["code_hashes"]
            or set(protocol["parent_file_hashes"]) != set(PARENT_FILES)):
        raise ValueError("分解结果与协议不一致")
    parent = Path(protocol["parent_path"])
    for root, hashes in [(code_root, protocol["code_hashes"]), (parent, protocol["parent_file_hashes"])]:
        for name, digest in hashes.items():
            if content_hash(root / name) != digest:
                raise ValueError(f"代码或父实验已变化：{name}")
    previous, parent_rows, archive = parent_read(parent)
    if (any(protocol[key] != previous[key] for key in INHERITED)
            or protocol["parent_code_root"] != str(archive) or protocol["parent_code_hashes"] != previous["code_hashes"]):
        raise ValueError("分解与父实验输入不一致")
    rows = observed["rows"]
    if [{key: row[key] for key in ("sample_id", "image_ref", "bbox")} for row in rows] != previous["samples"]:
        raise ValueError("分解样本缺失或乱序")
    for row in rows:
        if [variant["case_id"] for variant in row["variants"]] != [case["id"] for case in previous["cases"]]:
            raise ValueError("分解条件缺失或乱序")
    return protocol, rows, verify_numerics(protocol, rows, parent_rows)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["prepare", "report"])
    parser.add_argument("--parent", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not args.output.is_absolute() or (args.parent is not None and not args.parent.is_absolute()):
        parser.error("必须使用绝对路径")
    if args.action == "prepare":
        if args.parent is None:
            parser.error("需要父实验目录")
        prepare(args.parent, args.output)
    else:
        from decomposition_report import report
        report(args.output)
