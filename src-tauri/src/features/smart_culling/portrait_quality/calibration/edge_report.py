"""All-condition spatial-width diagnostics, including fit rejection and support loss."""

import json
from statistics import median

from candidate import finite
from edge_candidate import predict
from edge_experiment import read
from gap_experiment import read_stress_pair
from legacy import content_hash, utc_now
from prepare import write_new
from robust_report import summarize_robust


def summarize(rows, protocol, candidate):
    result = summarize_robust(rows, protocol, candidate, predictor=predict,
                             evidence_keys=("detail", "robust_detail", "edge_spread"))
    result["classifier_valid"] = candidate.get("fit_status") == "fitted"
    result["fit_status"] = candidate.get("fit_status")
    result["screening_passed"] &= result["classifier_valid"]
    ranges = {}
    for i, case in enumerate(protocol["cases"]):
        groups = {}
        for label in (1, 2):
            measured = [row["variants"][i].get("edge_spread") or {} for row in rows if row["label"] == label]
            values = {}
            for field in ["widthP25", "widthMedian", "widthP75", "acceptedProfiles", "contrastMedian"]:
                numbers = [m[field] for m in measured if finite(m.get(field))]
                values[field] = dict(missing=len(measured)-len(numbers), min=min(numbers) if numbers else None,
                    median=median(numbers) if numbers else None, max=max(numbers) if numbers else None)
            groups[str(label)] = values
        ranges[case["id"]] = groups
    result["numerical_ranges_by_original_label"] = ranges
    return result


def report(output):
    protocol, rows, candidate = read_stress_pair(output, read, "edge_spread")
    result = summarize(rows, protocol, candidate)
    result.update(schema="qraw-clarity-edge-spread-stress-report-4", created_at=utc_now(),
        protocol_sha256=content_hash(output / "PROTOCOL.json"), observations_sha256=content_hash(output / "observations.json"))
    write_new(output / "stress_report.json", result)
    triple = lambda counts: "/".join(str(counts.get(s, 0)) for s in ["clear", "unclear", "uncertain"])
    base = result["baseline"]
    lines = ["# 候选 0.4：局部边缘过渡宽度", "",
        f"预设筛查{'通过' if result['screening_passed'] else '未通过'}；拟合状态 {result['fit_status']}。正式门禁未替换。",
        f"原图同批正确 {base['correct']}/24，清晰误弃 {base['clear_false_discard']}/14，模糊误放 {base['blurry_false_pass']}/10，未知 {base['unknown']}/24。",
        "来源与类别完全混杂，独立留出 0；没有真实摄影泛化结论。", ""]
    if not result["classifier_valid"]:
        lines += ["没有有效分类器：全部未知只是空参数占位，不能用零误判包装性能。", ""]
    lines += ["24×16=384 次真实 Rust 测量，合成条件没有新人工真值。数值投射忽略来源限制，只供诊断；来源保护输出单独保存在 JSON。", "",
        "| 条件 | 原清晰 → 清晰/模糊/未知 | 原模糊 → 清晰/模糊/未知 |", "| --- | --- | --- |"]
    for case, counts in result["by_case"].items():
        lines.append(f"| {case} | {triple(counts['clear_origin'])} | {triple(counts['blurry_origin'])} |")
    lines += ["", f"噪声/锐化模糊数值误放：{result['noise_sharpen_promotions'] or '无'}。",
        f"每种噪声/锐化条件的误弃与未知底线：{result['noise_coverage_checks']}。",
        f"背景改变观测 {result['background_detail_changes']}/48，renderedRgb8 接受 {result['rendered_source_accepted']}/24。",
        "JSON 保留全部支持数量、拒绝原因、宽度分位数、对比度和各样本输出；未因侧脸、缺 FIQA 或缺 landmark 删除任何照片。",
        "固定原像素边缘宽度不是缩图不变量；低对比、曲线边缘、毛发和多个边缘可能破坏测量。测试图形通过不代表摄影通过。",
        "复用已有变体、像素来源检查、一次间隔拟合和全部筛查统计。只在 test 编译，不增加运行时评分路径。",
        "RAW、GPU、重新检测和完整桌面流程不由本实验验证；替换门禁前仍需未调参、来源交叉、人物/拍摄组隔离的独立留出，每类至少20张。", ""]
    with (output / "ASSESSMENT.md").open("x", encoding="utf-8") as f:
        f.write("\n".join(lines))
    print(json.dumps({k: result[k] for k in ["fit_status", "baseline", "screening_passed", "noise_sharpen_promotions", "noise_coverage_checks"]}, ensure_ascii=False, indent=2))
