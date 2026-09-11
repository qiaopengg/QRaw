"""Signed all-condition attribution; no fitted threshold or new accuracy claim."""

from collections import Counter, defaultdict
from statistics import median

from legacy import content_hash, utc_now
from noise_decomposition import checked_read
from prepare import write_new

SPATIAL = ("same_position", "different_position", "signal_noise_cross")


def statistics(values):
    absolute = sorted(abs(value) for value in values)
    return dict(count=len(values), median_signed=median(values), median_absolute=median(absolute),
        p90_absolute=absolute[int((len(absolute) - 1) * .9)], maximum_absolute=absolute[-1])


def analyze(protocol, rows):
    groups = defaultdict(lambda: defaultdict(list))
    dominant = defaultdict(Counter)
    conditions = []
    for row in rows:
        for case, variant in zip(protocol["cases"], row["variants"]):
            noise = next((s for s in case["stages"] if s["kind"] in ("noise", "shared_noise")), None)
            if noise is None:
                continue
            order = "noise_before_blur" if case["id"].endswith("_before_blur") else "noise_after_blur"
            if case["id"].endswith("_sharpen"):
                order += "_sharpen"
            scales, trace = variant["decomposition"]["scales"], variant["trace"]
            drift = [abs(s["observed"] - s["reference"]) / max(1.0, abs(s["reference"])) for s in scales]
            index = max(range(2), key=lambda i: drift[i])
            worst = scales[index]
            term = max(SPATIAL, key=lambda name: abs(worst[name]))
            dominant[noise["kind"]][term] += 1
            record = dict(sample_id=row["sample_id"], image_ref=row["image_ref"], case_id=case["id"],
                reference_id=case["reference_id"], noise=noise["kind"], amplitude=noise["value"], seed=case["seed"], order=order,
                negative=any(s["observed"] < 0 for s in scales), drift=drift, largest_drift_scale=index,
                dominant_spatial_term=term, scales=scales, trace=trace)
            conditions.append(record)
            for group in (noise["kind"], f"{noise['kind']}/amp{noise['value']}/{order}", f"{noise['kind']}/seed{case['seed']}"):
                for scale in range(2):
                    for name in SPATIAL:
                        groups[group][f"scale{scale}/{name}"].append(scales[scale][name])
                    for name, values in trace["terms"].items():
                        groups[group][f"scale{scale}/{name}"].append(values[scale])
    return dict(conditions=conditions, groups={group: {term: statistics(values) for term, values in terms.items()}
        for group, terms in groups.items()}, dominant_terms={key: dict(value) for key, value in dominant.items()},
        negative_conditions=sum(row["negative"] for row in conditions), candidate_fitted=False,
        classification_evaluable=False, new_candidate_created=False, release_ready=False)


def report(output):
    protocol, rows, errors = checked_read(output)
    result = dict(schema="qraw-noise-decomposition-report-1", created_at=utc_now(),
        protocol_sha256=content_hash(output / "PROTOCOL.json"), observations_sha256=content_hash(output / "observations.json"),
        base_count=len(rows), measured_conditions=sum(len(row["variants"]) for row in rows),
        identities_passed=True, maximum_errors=errors, **analyze(protocol, rows))
    write_new(output / "SUMMARY.json", result)
    examples = [max((row for row in result["conditions"] if row["noise"] == noise), key=lambda row: max(row["drift"]))
                for noise in ("noise", "shared_noise")]
    write_new(output / "COUNTEREXAMPLES.json", examples)
    lines = ["# 离线噪声误差分解：实际结果", "",
        f"{len(rows)} 张清晰基底、{result['measured_conditions']} 条件均完成；v1 数值回归与各恒等式通过。",
        "这只验证诊断自洽；没有抗噪分类器、人工模糊真值或独立准确率。", "",
        "| 噪声类型 | 最大偏移尺度：同位置项绝对值最大 | 异位置项最大 | 信号交叉项最大 |",
        "| --- | --- | --- | --- |"]
    for noise in ("noise", "shared_noise"):
        counts = result["dominant_terms"][noise]
        lines.append(f"| {noise} | {counts.get('same_position', 0)} | {counts.get('different_position', 0)} | {counts.get('signal_noise_cross', 0)} |")
    lines += ["", "每类分母 1,188；按每条件最大偏移尺度比较三项绝对值，存在相互抵消，不是错误概率或因果占比。",
        f"负共同能量条件 {result['negative_conditions']} 个，全部保留。逐条件、幅度、顺序和精确种子分组见 SUMMARY.json。", "",
        "| 反例 | 参考能量 | 观测能量 | 同位置 | 异位置 | 信号交叉 |", "| --- | --- | --- | --- | --- | --- |"]
    for row in examples:
        scale = row["scales"][row["largest_drift_scale"]]
        lines.append(f"| {row['image_ref']} / {row['case_id']} | {scale['reference']:.6f} | {scale['observed']:.6f} | {scale['same_position']:.6f} | {scale['different_position']:.6f} | {scale['signal_noise_cross']:.6f} |")
    lines += ["", "处理顺序的四项贡献和两路裁切比例见 COUNTEREXAMPLES.json；锐化滤波与内部模糊量化计入 processing，锐化输出截断另列 sharpen_clip。",
        "恒等式容差保持 1e-8；原始 v1 失败和代码归档未改，未生成 0.5 或启用正式评分。参考和已知噪声仅用于离线诊断，真实独立留出仍各 0/20。"]
    with (output / "REPORT.md").open("x", encoding="utf-8") as file:
        file.write("\n".join(lines) + "\n")
    print(f"已保存 {result['measured_conditions']} 条件；恒等式通过，仍无分类准确率")
