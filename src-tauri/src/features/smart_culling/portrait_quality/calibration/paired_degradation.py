"""Fixed, create-new paired degradation experiments on completed clarity originals."""

import argparse
import json
from pathlib import Path

from audit import verify
from legacy import content_hash, utc_now
from prepare import write_new
from stress import FEATURE

SIGMAS = (0, 1, 2, 4)
SEEDS = (0x51455741, 1, 20260910)
RULES = dict(identity_tolerance=1e-8, energy_drift_limit=0.20, minimum_stable_fraction=0.90)
LEGACY_FILES = ["portrait_quality.rs", "portrait_quality/tests.rs", "analysis.rs", "infrastructure/clarity_input.rs",
    "../../image_loader.rs", "../../image_processing.rs", "../../../Cargo.toml", "../../../Cargo.lock",
    *[f"portrait_quality/{name}.rs" for name in ["stress_variants", "stress_probe", "degradation_cases",
        "degradation_probe", "cross_channel_detail", "cross_channel_detail_tests", "detail", "robust_detail",
        "linear_response", "linear_detail", "edge_spread", "edge_profile", "face"]],
    *[f"portrait_quality/calibration/{name}.py" for name in ["paired_degradation", "paired_degradation_report",
        "test_paired_degradation", "audit", "labeler", "legacy"]]]
FILES = [*LEGACY_FILES, "portrait_quality/paired_probe_io.rs"]
PROTOCOL_SCHEMA = "qraw-paired-degradation-protocol-2"
OBSERVATIONS_SCHEMA = "qraw-paired-degradation-observations-2"


def matrix():
    cases = []
    stage = lambda kind, value: dict(id=kind, kind=kind, value=value)
    for sigma in SIGMAS:
        blur = [stage("blur", sigma)] if sigma else []
        reference = f"blur{sigma}"
        sharp_reference = reference + "_sharpen"
        sharp = [stage("unsharpen", 1.2)]
        cases += [dict(id=reference, reference_id=reference, seed=0, stages=blur),
                  dict(id=sharp_reference, reference_id=sharp_reference, seed=0, stages=blur + sharp)]
        for noise in ("noise", "shared_noise"):
            for amplitude in (8, 24, 48):
                for seed in SEEDS:
                    added = [stage(noise, amplitude)]
                    prefix = f"{reference}_{noise}{amplitude}_seed{seed}"
                    cases += [dict(id=prefix, reference_id=reference, seed=seed, stages=blur + added),
                              dict(id=prefix + "_sharpen", reference_id=sharp_reference, seed=seed,
                                   stages=blur + added + sharp)]
                    if sigma:
                        cases.append(dict(id=prefix + "_before_blur", reference_id=reference,
                                          seed=seed, stages=added + blur))
    return cases


def prepare(frozen, output, *, expected_count=12):
    completion = verify(frozen, expected_count=expected_count)
    snapshot = json.loads((frozen / "FREEZE_REPORT.json").read_text())
    if snapshot["file_hashes"] != completion["file_hashes"]:
        raise ValueError("冻结数据哈希不一致")
    if completion["distribution"] != {"清晰": expected_count}:
        raise ValueError("本实验要求事前完成的清晰基底，不自动过滤或重标")
    manifest = json.loads((frozen / "manifest.json").read_text())
    protocol = dict(schema=PROTOCOL_SCHEMA, created_at=utc_now(),
        frozen_path=str(frozen), data_version=snapshot["data_version"], sample_count=expected_count,
        samples=[{key: item[key] for key in ("sample_id", "image_ref", "bbox")} for item in manifest["items"]],
        frozen_file_hashes=dict(completion["file_hashes"], **{"FREEZE_REPORT.json": content_hash(frozen / "FREEZE_REPORT.json")}),
        code_hashes={name: content_hash(FEATURE / name) for name in FILES},
        design_sha256=content_hash(output / "DESIGN.md"), cases=matrix(), rules=RULES,
        original_label_usage="completed clear bases only; no inherited labels for degradations",
        source_provenance=sorted({item.get("provenance", "unverified") for item in manifest["items"]}),
        independent_holdout=False, candidate_fitted=False, runtime_enabled=False, release_ready=False)
    write_new(output / "PROTOCOL.json", protocol)
    return protocol


def checked_read(output, *, code_root=FEATURE):
    protocol = json.loads((output / "PROTOCOL.json").read_text())
    observed = json.loads((output / "observations.json").read_text())
    legacy = protocol["schema"] == "qraw-paired-degradation-protocol-1"
    if (protocol["schema"] not in ("qraw-paired-degradation-protocol-1", PROTOCOL_SCHEMA)
            or observed["schema"] != ("qraw-paired-degradation-observations-1" if legacy else OBSERVATIONS_SCHEMA)
            or protocol["cases"] != matrix() or protocol["rules"] != RULES
            or set(protocol["code_hashes"]) != set(LEGACY_FILES if legacy else FILES)
            or observed["code_hashes"] != protocol["code_hashes"]
            or observed["protocol_sha256"] != content_hash(output / "PROTOCOL.json")
            or protocol["design_sha256"] != content_hash(output / "DESIGN.md")):
        raise ValueError("观测、矩阵或代码协议不一致")
    for name, digest in protocol["code_hashes"].items():
        if content_hash(code_root / name) != digest:
            raise ValueError(f"测量代码已变化：{name}")
    frozen = Path(protocol["frozen_path"])
    for name, digest in protocol["frozen_file_hashes"].items():
        if content_hash(frozen / name) != digest:
            raise ValueError(f"冻结输入已变化：{name}")
    manifest = json.loads((frozen / "manifest.json").read_text())
    samples = [{key: item[key] for key in ("sample_id", "image_ref", "bbox")} for item in manifest["items"]]
    if protocol["samples"] != samples or len(samples) != protocol["sample_count"]:
        raise ValueError("样本协议与冻结清单不一致")
    rows = observed["rows"]
    if [{key: row[key] for key in ("sample_id", "image_ref", "bbox")} for row in rows] != samples:
        raise ValueError("观测样本缺失、重复或顺序错误")
    for item, row in zip(manifest["items"], rows):
        if content_hash(Path(item["source_path"])) != item["sample_id"]:
            raise ValueError("原始基底已变化")
        if [variant["case_id"] for variant in row["variants"]] != [case["id"] for case in matrix()]:
            raise ValueError("条件缺失、重复或顺序错误")
    return protocol, rows


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["prepare", "report"])
    parser.add_argument("--frozen", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if not args.output.is_absolute() or (args.frozen is not None and not args.frozen.is_absolute()):
        parser.error("必须使用绝对路径")
    if args.action == "prepare":
        if args.frozen is None:
            parser.error("需要冻结基底")
        protocol = prepare(args.frozen, args.output)
        print(f"已固定 {protocol['sample_count']}×{len(protocol['cases'])} 条件，尚未测量")
    else:
        from paired_degradation_report import report
        report(args.output)
