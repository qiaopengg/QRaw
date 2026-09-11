"""Run baseline -> fit once -> fixed stress experiment; outputs are create-new only."""

import argparse
import json
from pathlib import Path

from candidate import metrics
from legacy import content_hash
from prepare import write_new
from robust_candidate import fit, predict
from robust_io import prepare_protocol, read_experiment, read_json


def fit_baseline(output):
    protocol, rows = read_experiment(output)
    if protocol["phase"] != "baseline":
        raise ValueError("禁止从抗扰结果重新拟合")
    anchors = [dict(row["variants"][0], label=row["label"], sample_id=row["sample_id"], image_ref=row["image_ref"]) for row in rows]
    candidate = fit(anchors)
    measured = metrics(anchors, [predict(row, candidate) for row in anchors])
    write_new(output / "candidate.json", candidate)
    write_new(output / "diagnosis.json", dict(candidate_parameters=candidate, baseline=measured,
        observations_sha256=content_hash(output / "observations.json"),
        protocol_sha256=content_hash(output / "PROTOCOL.json"), rows=anchors,
        source_confounded=True, independent_holdout_count=0, runtime_enabled=False, release_ready=False))
    with (output / "DIAGNOSIS_REPORT.md").open("x", encoding="utf-8") as target:
        target.write(f"# 清晰度候选 0.2：原图拟合诊断\n\n"
            f"固定 5×5 灰度中值滤波后复用 0.1 极值间隔拟合方法；noiseSigma 只记录，不用于门槛。\n\n"
            f"正确 {measured['correct']}/{measured['total']}，清晰误弃 {measured['clear_false_discard']}，"
            f"模糊误放 {measured['blurry_false_pass']}，未知 {measured['unknown']}。\n\n"
            "24 张均为已知校准照片，来源与答案完全混杂，无独立人物/拍摄组留出；不能称为泛化成绩。"
            "候选与参数在抗扰测量前固定，不进入正式评分。\n")
    print(json.dumps(measured, ensure_ascii=False, indent=2))


def prepare_stress(baseline, output):
    protocol, _ = read_experiment(baseline)
    candidate = read_json(baseline / "candidate.json")
    diagnosis = read_json(baseline / "diagnosis.json")
    if (protocol["phase"] != "baseline" or candidate != diagnosis["candidate_parameters"]
            or diagnosis["observations_sha256"] != content_hash(baseline / "observations.json")
            or diagnosis["protocol_sha256"] != content_hash(baseline / "PROTOCOL.json")):
        raise ValueError("候选或原图基线已改变")
    prepare_protocol(Path(protocol["frozen_path"]), output, "stress", baseline_dir=str(baseline),
        baseline_observations_sha256=content_hash(baseline / "observations.json"),
        candidate_sha256=content_hash(baseline / "candidate.json"))
    write_new(output / "candidate.json", candidate)
    print(f"0.2 参数及 24×16 矩阵已固定：{output}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["baseline", "fit", "stress", "report"])
    parser.add_argument("--frozen", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if any(path is not None and not path.is_absolute() for path in [args.frozen, args.baseline, args.output]):
        parser.error("必须使用绝对路径")
    if args.action == "baseline":
        if args.frozen is None:
            parser.error("需要冻结批次路径")
        prepare_protocol(args.frozen, args.output, "baseline")
        print(f"已固定 24 张原图测量；尚未拟合：{args.output}")
    elif args.action == "fit":
        fit_baseline(args.output)
    elif args.action == "stress":
        if args.baseline is None:
            parser.error("需要候选原图基线路径")
        prepare_stress(args.baseline, args.output)
    else:
        from robust_report import report
        report(args.output)
