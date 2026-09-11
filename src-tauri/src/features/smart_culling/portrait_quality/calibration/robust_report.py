"""0.2 reporting reuses the stress summarizer, retaining failures and abstentions."""

import json
from pathlib import Path

from legacy import content_hash, utc_now
from prepare import write_new
from robust_candidate import predict
from robust_io import read_experiment, read_json
from stress_summary import same_detail, summarize


def summarize_robust(rows, protocol, candidate, predictor=predict,
                     evidence_keys=("detail", "robust_detail")):
    result = summarize(rows, protocol, candidate, predictor=predictor, evidence_keys=evidence_keys)
    checks = protocol["checks"]
    noise_checks = {}
    for case in protocol["cases"]:
        if case["kind"] not in {"noise", "unsharpen", "noise_sharpen"}:
            continue
        counts = result["by_case"][case["id"]]
        unknown = sum(counts[origin].get("uncertain", 0) for origin in ["clear_origin", "blurry_origin"])
        clear_to_unclear = counts["clear_origin"].get("unclear", 0)
        noise_checks[case["id"]] = dict(unknown=unknown, total=counts["total"], clear_to_unclear=clear_to_unclear,
            passed=unknown / counts["total"] <= checks["noise_unknown_rate_max"]
                   and clear_to_unclear <= checks["noise_clear_to_unclear_allowed"])
    result["noise_coverage_checks"] = noise_checks
    result["screening_passed"] &= result["baseline"]["calibration_floor_passed"] and all(c["passed"] for c in noise_checks.values())
    result["runtime_enabled"] = False
    return result


def report(output):
    protocol, rows = read_experiment(output)
    if protocol["phase"] != "stress":
        raise ValueError("不是完整抗扰实验")
    baseline_dir = Path(protocol["baseline_dir"])
    if (content_hash(output / "candidate.json") != protocol["candidate_sha256"]
            or content_hash(baseline_dir / "observations.json") != protocol["baseline_observations_sha256"]):
        raise ValueError("固定参数或基线变化")
    _, baseline = read_experiment(baseline_dir)
    lookup = {row["sample_id"]: row["variants"][0] for row in baseline}
    for row in rows:
        if not same_detail(row["variants"][0]["robust_detail"], lookup[row["sample_id"]]["robust_detail"], protocol["detail_absolute_tolerance"]):
            raise ValueError("抗噪基线复测不同")
    result = summarize_robust(rows, protocol, read_json(output / "candidate.json"))
    result.update(schema="qraw-clarity-robust-stress-report-2", created_at=utc_now(),
        protocol_sha256=content_hash(output / "PROTOCOL.json"), observations_sha256=content_hash(output / "observations.json"))
    write_new(output / "stress_report.json", result)
    with (output / "STRESS_REPORT.md").open("x", encoding="utf-8") as target:
        target.write(markdown(result))
    print(json.dumps({key: result[key] for key in ["baseline", "noise_sharpen_promotions", "noise_coverage_checks", "screening_passed"]}, ensure_ascii=False, indent=2))


def markdown(result):
    base = result["baseline"]
    triple = lambda counts: "/".join(str(counts.get(state, 0)) for state in ["clear", "unclear", "uncertain"])
    lines = ["# 清晰度候选 0.2：固定中值滤波抗扰诊断", "",
        f"预设筛查：{'通过' if result['screening_passed'] else '未通过'}；候选未启用正式评分。",
        f"原图正确 {base['correct']}/{base['total']}，清晰误弃 {base['clear_false_discard']}/14，模糊误放 {base['blurry_false_pass']}/10，未知 {base['unknown']}/24。", "",
        "24 张原图均为已知校准数据，无独立留出。16 个条件共 384 次 Rust 测量；原图重测对照历史细节与新基线。",
        "下表清晰/模糊标签仅属于原图，合成变体不新增人工真值；数值投射忽略来源限制，来源门禁输出另存 JSON。", "",
        "| 条件 | 原图清晰 14 张 → 清晰/模糊/未知 | 原图模糊 10 张 → 清晰/模糊/未知 | 改变 |",
        "| --- | --- | --- | --- |"]
    for name, case in result["by_case"].items():
        lines.append(f"| {name} | {triple(case['clear_origin'])} | {triple(case['blurry_origin'])} | {case['changed']}/{case['total']} |")
    lines += ["", f"框外背景导致局部观测改变：{result['background_detail_changes']}/48。",
        f"噪声/锐化使原本模糊升为清晰的样本：{result['noise_sharpen_promotions'] or '无'}。",
        f"每个噪声/锐化条件的未知不得超过 20%，按原图清晰标签对照的降为模糊数须为 0：{result['noise_coverage_checks']}。这是防止全弃权包装成功的筛查约束，不是变体真实准确率。",
        f"renderedRgb8 来源接受 {result['rendered_source_accepted']}/24；拒绝来源不能抵消数值抗噪失败。", "",
        "5×5 中值滤波固定后仍复用原局部细节和间隔拟合方法。noiseSigma 仅为高通响应尺度估计，不是相机噪声概率，未事后选择噪声阈值。",
        "人物/拍摄组未隔离，来源与标签混杂；RAW、GPU 渲染、重新检测和完整桌面交互未验收，不能宣称真实摄影泛化。",
        "0.1 参数和失败报告、原图及历史标签均保留。0.2 不原地调参遮掩本次结果；正式评分门禁没有接入该候选，光学/构图仍 +0。", ""]
    return "\n".join(lines)
