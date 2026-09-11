"""Persist every failure without promoting a numerical probe to a focus classifier."""

import json
from pathlib import Path

from legacy import content_hash, utc_now
from observed_noise import checked_read
from observed_noise_metrics import analyze
from prepare import write_new


def report(output):
    protocol, rows, prior, checks = checked_read(output)
    result = dict(schema="qraw-observed-noise-report-1", created_at=utc_now(),
        protocol_sha256=content_hash(output / "PROTOCOL.json"), observations_sha256=content_hash(output / "observations.json"),
        base_count=len(rows), measured_conditions=sum(len(r["variants"]) for r in rows), maximum_errors=checks,
        **analyze(protocol, rows, prior))
    failures = [r for r in result["conditions"] if not (r["stable"]["corrected_energy"] if r["noise"] == "clean" else all(r["stable"].values()))]
    old = json.loads((Path(protocol["parent_path"]) / "COUNTEREXAMPLES.json").read_text())
    keys = {(r["sample_id"], r["case_id"]) for r in old}
    examples = [r for r in result["conditions"] if (r["sample_id"], r["case_id"]) in keys]
    if len(examples) != len(keys):
        raise ValueError("原失败反例缺失")
    write_new(output / "SUMMARY.json", result)
    write_new(output / "FAILURES.json", failures)
    write_new(output / "COUNTEREXAMPLES.json", examples)
    lines = ["# 观测像素噪声估计：固定方法结果", "",
        f"{result['base_count']} 张基底、{result['measured_conditions']} 条件，原观测回归及代理/分块数值检查通过。",
        f"机制筛查通过：{result['screening_passed']}；无噪声误扣控制通过：{result['clean_leakage_passed']}。", "",
        "| 条件 | 数量 | 同位置误差达标 | 异位置误差达标 | 总残差误差达标 | 校正能量稳定 | 原能量稳定 |", "| --- | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for kind in ("clean", "noise", "shared_noise"):
        group = result["groups"][kind]
        lines.append(f"| {kind} | {group['count']} | {group['stable']['same_position']} | {group['stable']['different_position']} | {group['stable']['residual_common']} | {group['stable']['corrected_energy']} | {group['original_stable']} |")
    lines += ["", "每条件取两尺度最差相对误差，≤20% 为达标；独立/共享噪声各项需至少90%，96个无噪声校正控制需全部达标。没有拟合类别阈值。", "",
        "| 噪声类型 | 总残差误差落在 ±2×分块标准误内 | 原负能量 | 负残差估计 | 负校正能量 |", "| --- | ---: | ---: | ---: | ---: |"]
    for kind in ("noise", "shared_noise"):
        group = result["groups"][kind]
        lines.append(f"| {kind} | {group['within_two_block_se']}/{group['count']} | {group['negative_original']} | {group['negative_estimate']} | {group['negative_corrected']} |")
    lines += ["", "分块标准误仅反映空间删除块敏感性，不能覆盖结构代理偏差或保证95%统计覆盖。三个固定种子的逐组误差均值/标准差见 SUMMARY.json 的 seed_variation，不当作独立照片。",
        f"全部 {len(failures)} 个未达机制条件的记录保存在 FAILURES.json；原 S012 反例见 COUNTEREXAMPLES.json。",
        "", "校正只扣代理残差共同能量，不扣代理结构交叉项。原图、标签、原抗噪 v1 与分解 v1/v2 未覆盖；本轮没有可用清晰度分类器、独立准确率或正式评分改动。"]
    with (output / "REPORT.md").open("x", encoding="utf-8") as file:
        file.write("\n".join(lines) + "\n")
    print(f"已保存 {result['measured_conditions']} 条；screening_passed={result['screening_passed']}；失败 {len(failures)} 条")
    return result
