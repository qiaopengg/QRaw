"""Paired measurement stability, never inherited human blur labels or accuracy."""

import math
from collections import defaultdict
from statistics import median

from legacy import content_hash, utc_now
from paired_degradation import RULES, SIGMAS, checked_read
from prepare import write_new


def vectors(variant):
    raw = variant.get("detail") or {}
    robust = variant.get("robust_detail") or {}
    denoised = robust.get("denoisedDetail") or {}
    linear = variant.get("linear_detail") or {}
    cross = variant.get("cross_channel") or {}
    edge = variant.get("edge_spread") or {}
    return dict(raw=[raw.get("nativeLaplacian"), raw.get("scaledLaplacian")],
        median=[denoised.get("nativeLaplacian"), denoised.get("scaledLaplacian")],
        linear=[scale.get("correctedEnergy") for scale in linear.get("scales", [])],
        cross=[scale.get("commonEnergy") for scale in cross.get("scales", [])],
        edge=[edge.get("widthP25")])


def drift(values, reference):
    if (not values or len(values) != len(reference)
            or any(value is None or not math.isfinite(value) for value in [*values, *reference])):
        return None
    return max(abs(value - base) / max(1.0, abs(base)) for value, base in zip(values, reference))


def summarize(values):
    valid = sorted(value for value in values if value is not None)
    stable = sum(value <= RULES["energy_drift_limit"] for value in valid)
    return dict(total=len(values), available=len(valid), stable=stable,
        stable_fraction=stable / len(values) if values else None,
        median_drift=median(valid) if valid else None,
        p90_drift=valid[int((len(valid) - 1) * 0.9)] if valid else None,
        maximum_drift=max(valid) if valid else None)


def analyze(protocol, rows):
    groups = defaultdict(lambda: defaultdict(list))
    conditions, clean_order = [], []
    for row in rows:
        variants = {variant["case_id"]: variant for variant in row["variants"]}
        for case in protocol["cases"]:
            added = next((stage for stage in case["stages"] if stage["kind"] in ("noise", "shared_noise")), None)
            if added is None:
                continue
            order = ("noise_before_blur" if case["stages"][0]["kind"] in ("noise", "shared_noise")
                     and any(stage["kind"] == "blur" for stage in case["stages"]) else "noise_after_blur")
            if case["stages"][-1]["kind"] == "unsharpen":
                order += "_sharpen"
            values = vectors(variants[case["id"]])
            reference = vectors(variants[case["reference_id"]])
            deltas = {name: drift(vector, reference[name]) for name, vector in values.items()}
            for name, delta in deltas.items():
                groups[added["kind"]][name].append(delta)
                groups[f"{added['kind']}/amp{added['value']}/{order}"][name].append(delta)
            conditions.append(dict(sample_id=row["sample_id"], image_ref=row["image_ref"], case_id=case["id"],
                reference_id=case["reference_id"], noise=added["kind"], amplitude=added["value"], order=order,
                drift=deltas, cross_negative=any(value is not None and value < 0 for value in values["cross"])))
        energies = [vectors(variants[f"blur{sigma}"])["cross"] for sigma in SIGMAS]
        available = all(len(values) == 2 and all(value is not None and math.isfinite(value) for value in values) for values in energies)
        ordered = available and all(right <= left + RULES["identity_tolerance"]
            for before, after in zip(energies, energies[1:]) for left, right in zip(before, after))
        clean_order.append(dict(sample_id=row["sample_id"], image_ref=row["image_ref"], energies=energies,
                                available=available, nonincreasing=ordered))
    summaries = {group: {name: summarize(values) for name, values in methods.items()} for group, methods in groups.items()}
    passed = bool(clean_order) and all(row["nonincreasing"] for row in clean_order)
    passed = passed and all((summaries.get(noise, {}).get("cross", {}).get("stable_fraction") or 0)
                           >= RULES["minimum_stable_fraction"] for noise in ("noise", "shared_noise"))
    return dict(groups=summaries, clean_blur_order=clean_order, conditions=conditions,
        stability_screening_passed=bool(passed), classification_evaluable=False, candidate_fitted=False,
        new_candidate_created=False, independent_holdout_count=0, release_ready=False)


def report(output):
    protocol, rows = checked_read(output)
    result = dict(schema="qraw-paired-degradation-report-1", created_at=utc_now(),
        protocol_sha256=content_hash(output / "PROTOCOL.json"), observations_sha256=content_hash(output / "observations.json"),
        base_count=len(rows), measured_conditions=sum(len(row["variants"]) for row in rows), **analyze(protocol, rows))
    write_new(output / "SUMMARY.json", result)
    lines = ["# 新基底受控退化与抗噪：实际结果", "",
        f"{len(rows)} 张清晰基底，{result['measured_conditions']} 次测量；不生成或继承人工模糊标签。", "",
        "| 方法 | 独立 RGB 噪声稳定条件 | 共享通道噪声稳定条件 |", "| --- | --- | --- |"]
    for name in ("raw", "median", "linear", "cross", "edge"):
        values = [result["groups"][noise][name] for noise in ("noise", "shared_noise")]
        lines.append(f"| {name} | {values[0]['stable']}/{values[0]['total']} | {values[1]['stable']}/{values[1]['total']} |")
    lines += ["", "raw/median 为原细节/中值细节，linear 为 0.3 校正能量，cross 为新跨通道共同能量，edge 为 0.4 边缘宽度。不同量的数值尺度不同，此表只比较各自相对同处理干净参考的稳定性，不能等同清晰度精度。",
        "", f"干净模糊能量顺序：{sum(row['nonincreasing'] for row in result['clean_blur_order'])}/{len(rows)} 非递增。",
        f"按事前条件的整体稳定性初筛：{'通过' if result['stability_screening_passed'] else '未通过'}。所有缺测、负值、种子、顺序和每图结果见 SUMMARY.json。",
        "", "这不是分类准确率。共享噪声、有限小脸裁片、量化/截断及颜色差异均可能使新假设失效；不以某一噪声分组的改善代替全部条件通过，不据此调整正式阈值。",
        "", "新代码仅 cfg(test)/离线；原图、冻结标签、旧候选与正式评分均保持。真实独立留出仍各 0/20，未创建分类候选 0.5。"]
    with (output / "REPORT.md").open("x", encoding="utf-8") as file:
        file.write("\n".join(lines) + "\n")
    print(f"已保存 {result['measured_conditions']} 条件；稳定性初筛={result['stability_screening_passed']}")
