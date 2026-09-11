"""All-condition noise decomposition summary, not a photographic quality verdict."""

import json
from collections import defaultdict
from statistics import median

from legacy import content_hash, utc_now
from prepare import write_new
from noise_mechanism import checked_read


def summarize(rows):
    grouped = defaultdict(list)
    for row in rows:
        case = row["case"]
        grouped[(case["processing"], case["panel"], case["amplitude"])].append(row)
    groups = []
    for (processing, panel, amplitude), members in grouped.items():
        measurements = [row["measurement"] for row in members]
        scales = []
        for index in range(2):
            values = [m["scales"][index] for m in measurements]
            selected = ["variance_estimation_error", "white_model_error", "cross_term", "signal_energy",
                        "noise_energy", "estimated_variance_corrected", "true_variance_white_corrected", "oracle_residual_corrected"]
            ranges = {name: dict(min=min(v[name] for v in values), median=median(v[name] for v in values),
                                max=max(v[name] for v in values)) for name in selected}
            ranges.update(order=values[0]["order"], negative_estimated=sum(v["estimated_variance_corrected"] <= 0 for v in values),
                          negative_true_white=sum(v["true_variance_white_corrected"] <= 0 for v in values))
            scales.append(ranges)
        groups.append(dict(processing=processing, panel=panel, amplitude=amplitude, count=len(members), scales=scales,
            missing_ratio=sum(m["linear_detail"]["coarseToFineRatio"] is None for m in measurements),
            variance_relative_error_median=median(m["variance_estimation_relative_error"] for m in measurements),
            lag1_correlation_median=[median(m["lag1_correlation"][index] for m in measurements) for index in range(2)]))
    return dict(groups=groups, row_count=len(rows), labels_used=False, candidate_created=False,
        max_linearity_error=max(s["linearity_error"] for r in rows for s in r["measurement"]["scales"]),
        max_decomposition_relative_error=max(abs(s["decomposition_error"]) / max(1.0, s["observed_energy"])
                                           for r in rows for s in r["measurement"]["scales"]), accuracy_validated=False)


def report(output):
    _, rows = checked_read(output)
    summary = summarize(rows)
    summary.update(observations_sha256=content_hash(output / "observations.json"), created_at=utc_now())
    write_new(output / "SUMMARY.json", summary)
    lines = ["# 噪声机制诊断 v1：全部固定条件", "",
        f"完成 {len(rows)} 个合成条件、每条件两个尺度。没有读取照片类别、拟合新候选或写入星级。",
        f"最大线性分解误差 {summary['max_linearity_error']:.3g}；最大相对能量分解误差 {summary['max_decomposition_relative_error']:.3g}。", "",
        "下表覆盖所有处理/面板/噪声分组；每组含全部预设尺寸和种子。负校正与缺比值只描述当前能量方法，不是清晰度类别或准确率。", "",
        "| 处理 | 面板 | 噪声 ± | 条件数 | 估计方差相对误差中位数 | 细尺度负校正数 | 缺比值数 | 水平残差相关中位数 |",
        "| --- | --- | --- | --- | --- | --- | --- | --- |"]
    for group in summary["groups"]:
        lines.append(f"| {group['processing']} | {group['panel']} | {group['amplitude']} | {group['count']} | "
            f"{group['variance_relative_error_median']:.3%} | {group['scales'][0]['negative_estimated']} | "
            f"{group['missing_ratio']} | {group['lag1_correlation_median'][0]:.4f} |")
    lines += ["", "SUMMARY.json 含所有组的白噪声模型误差、估计误差、交叉项和三种扣除结果范围；observations.json 保留逐个尺寸/种子的完整分解。",
        "真实残差、干净参考均为合成诊断已知量，真实单张照片不可取得。分解自洽不代表可用清晰度算法；仍需解释这些误差并保留独立摄影验收要求。", ""]
    with (output / "REPORT.md").open("x", encoding="utf-8") as target:
        target.write("\n".join(lines))
    print(json.dumps({key: summary[key] for key in ["row_count", "max_linearity_error", "max_decomposition_relative_error"]}, indent=2))
