"""Observed-only noise experiment, compared with a verified archived oracle offline."""

import argparse
import json
from pathlib import Path

import noise_decomposition as decomposition
from legacy import content_hash, utc_now
from prepare import write_new
from stress import FEATURE

VERSION = "qraw-observed-rgb-median-residual-1"
RULES = dict(identity_tolerance=1e-8, relative_error_limit=0.20, minimum_fraction=0.90,
             clean_required_fraction=1.0, median_radius=2, block_grid=[4, 4], sensitivity_multiplier=2.0)
FILES = [*decomposition.FILES, *[f"portrait_quality/{name}.rs" for name in (
    "observed_noise", "observed_noise_tests", "observed_noise_probe", "noise_sampling")],
    *[f"portrait_quality/calibration/{name}.py" for name in (
        "observed_noise", "observed_noise_checks", "observed_noise_metrics", "observed_noise_report", "test_observed_noise")]]
PARENT_FILES = ("PROTOCOL.json", "DESIGN.md", "observations.json", "SUMMARY.json", "COUNTEREXAMPLES.json",
                "CONCLUSIONS.md", "REPORT.md", "VALIDATION.json", "CODE_MAP.json")


def parent_read(parent):
    return decomposition.parent_read(parent, reader=decomposition.checked_read)


def prepare(parent, output):
    previous, _, archive = parent_read(parent)
    protocol = dict(schema="qraw-observed-noise-protocol-1", created_at=utc_now(), estimator_version=VERSION,
        **{key: previous[key] for key in decomposition.INHERITED}, rules=RULES,
        parent_path=str(parent), parent_code_root=str(archive), parent_code_hashes=previous["code_hashes"],
        parent_file_hashes={name: content_hash(parent / name) for name in PARENT_FILES},
        code_hashes={name: content_hash(FEATURE / name) for name in FILES}, design_sha256=content_hash(output / "DESIGN.md"),
        estimator_input="observedRgb8Only", reference_usage="offline evaluation only",
        candidate_fitted=False, runtime_enabled=False, release_ready=False)
    write_new(output / "PROTOCOL.json", protocol)
    print(f"已固定 {protocol['sample_count']}×{len(protocol['cases'])}；未运行观测噪声估计")
    return protocol


def checked_read(output):
    from observed_noise_checks import verify
    protocol = json.loads((output / "PROTOCOL.json").read_text())
    observed = json.loads((output / "observations.json").read_text())
    if (protocol["schema"] != "qraw-observed-noise-protocol-1" or protocol["rules"] != RULES
            or protocol["estimator_version"] != VERSION or protocol["estimator_input"] != "observedRgb8Only"
            or observed["schema"] != "qraw-observed-noise-observations-1"
            or observed["estimator_clean_reference_used"] is not False
            or observed["protocol_sha256"] != content_hash(output / "PROTOCOL.json")
            or observed["code_hashes"] != protocol["code_hashes"] or set(protocol["code_hashes"]) != set(FILES)
            or protocol["design_sha256"] != content_hash(output / "DESIGN.md")
            or set(protocol["parent_file_hashes"]) != set(PARENT_FILES)):
        raise ValueError("观测噪声协议或边界不一致")
    parent = Path(protocol["parent_path"])
    for root, hashes in [(FEATURE, protocol["code_hashes"]), (parent, protocol["parent_file_hashes"])]:
        for name, digest in hashes.items():
            if content_hash(root / name) != digest:
                raise ValueError(f"代码或父实验变化：{name}")
    previous, prior, archive = parent_read(parent)
    if (any(protocol[key] != previous[key] for key in decomposition.INHERITED)
            or protocol["parent_code_root"] != str(archive) or protocol["parent_code_hashes"] != previous["code_hashes"]):
        raise ValueError("估计实验与父实验输入不一致")
    rows = observed["rows"]
    if [{key: row[key] for key in ("sample_id", "image_ref", "bbox")} for row in rows] != previous["samples"]:
        raise ValueError("估计样本缺失或乱序")
    for row in rows:
        if [v["case_id"] for v in row["variants"]] != [c["id"] for c in protocol["cases"]]:
            raise ValueError("估计条件缺失或乱序")
    return protocol, rows, prior, verify(protocol, rows, prior)


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
            parser.error("需要已核验的分解目录")
        prepare(args.parent, args.output)
    else:
        from observed_noise_report import report
        report(args.output)
