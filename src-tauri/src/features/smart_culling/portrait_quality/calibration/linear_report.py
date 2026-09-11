"""0.3 sensitivity report. Fit rejection and poor coverage can never pass screening."""

import json
from statistics import median

from candidate import finite
from legacy import content_hash, utc_now
from prepare import write_new
from linear_candidate import predict
from linear_experiment import read
from gap_experiment import read_stress_pair
from robust_report import summarize_robust


def summarize(rows, protocol, candidate):
    result = summarize_robust(rows, protocol, candidate, predictor=predict,
                             evidence_keys=("detail", "robust_detail", "linear_detail"))
    result["classifier_valid"] = candidate.get("fit_status") == "fitted"
    result["fit_status"] = candidate.get("fit_status")
    result["screening_passed"] &= result["classifier_valid"]
    numerical = {}
    for index, case in enumerate(protocol["cases"]):
        groups = {}
        for label in (1, 2):
            observations = [row["variants"][index].get("linear_detail") or {} for row in rows if row["label"] == label]
            ranges = {}
            for name, getter in {
                "noise_sigma": lambda m: m.get("noiseSigma"),
                "fine_observed": lambda m: (m.get("scales") or [{}, {}])[0].get("observedEnergy"),
                "fine_corrected": lambda m: (m.get("scales") or [{}, {}])[0].get("correctedEnergy"),
                "coarse_corrected": lambda m: (m.get("scales") or [{}, {}])[1].get("correctedEnergy"),
                "coarse_to_fine_ratio": lambda m: m.get("coarseToFineRatio"),
            }.items():
                values = [getter(measured) for measured in observations]
                valid = [value for value in values if finite(value)]
                ranges[name] = dict(missing=len(values) - len(valid), min=min(valid) if valid else None,
                                    median=median(valid) if valid else None, max=max(valid) if valid else None)
            groups[str(label)] = ranges
        numerical[case["id"]] = groups
    result["numerical_ranges_by_original_label"] = numerical
    return result


def report(output):
    protocol, rows, candidate = read_stress_pair(output, read, "linear_detail")
    result = summarize(rows, protocol, candidate)
    result.update(schema="qraw-clarity-linear-noise-stress-report-3", created_at=utc_now(),
        protocol_sha256=content_hash(output / "PROTOCOL.json"), observations_sha256=content_hash(output / "observations.json"))
    write_new(output / "stress_report.json", result)
    with (output / "ASSESSMENT.md").open("x", encoding="utf-8") as target:
        target.write(markdown(result))
    print(json.dumps({key: result[key] for key in ["classifier_valid", "baseline", "screening_passed",
        "noise_sharpen_promotions", "noise_coverage_checks"]}, ensure_ascii=False, indent=2))


def markdown(result):
    base = result["baseline"]
    triple = lambda counts: "/".join(str(counts.get(state, 0)) for state in ["clear", "unclear", "uncertain"])
    lines = ["# 候选 0.3：线性噪声能量校正", "",
        f"**预设筛查{'通过' if result['screening_passed'] else '未通过'}，正式门禁未替换。** 拟合状态：{result['fit_status']}。",
        f"原图同批正确 {base['correct']}/24，清晰误弃 {base['clear_false_discard']}/14，模糊误放 {base['blurry_false_pass']}/10，未知 {base['unknown']}/24。",
        "原图来源与类别完全混杂，独立留出 0；不能解释为泛化准确率。", ""]
    if not result["classifier_valid"]:
        lines += ["**没有有效分类器：下表全未知是拟合拒绝占位，0 误放/误弃或来源接受 0 都不能证明性能。**", ""]
    lines += ["## 原矩阵 24×16＝384 次 Rust 测量", "",
        "下面两组只沿用原图标签，合成变体不是新人工真值。数值投射忽略来源限制，仅用于敏感性；来源保护输出另存 JSON。", "",
        "| 条件 | 原清晰 → 清晰/模糊/未知 | 原模糊 → 清晰/模糊/未知 |", "| --- | --- | --- |"]
    for name, counts in result["by_case"].items():
        lines.append(f"| {name} | {triple(counts['clear_origin'])} | {triple(counts['blurry_origin'])} |")
    lines += ["", f"框外背景改变局部证据：{result['background_detail_changes']}/48；renderedRgb8 接受：{result['rendered_source_accepted']}/24。",
        f"噪声/锐化使原模糊数值升清晰：{result['noise_sharpen_promotions'] or '无'}。",
        f"防止误弃和全部弃权的预设检查（每种条件清晰降模糊 0，未知率 ≤20%）：{result['noise_coverage_checks']}。", "",
        "## 数值诊断与限制", "",
        "JSON 的 numerical_ranges_by_original_label 保存两组全部条件下扣除前/后细节能量、噪声估计和尺度比值的范围、中位数、缺测数量；rows 保留每张结果与负校正值。",
        "线性白噪声能量恒等式与噪声估计是否适用于照片是两件事；纹理、压缩、裁切、锐化相关噪声可能导致过扣或欠扣。负校正不当作模糊，尺度比值缺失进入未知。",
        "复用既有 EXIF/脸框检查、噪声估计、变体生成、极值间隔拟合及筛查统计；没有以模型低分、侧脸或缺 landmark 删除样本。",
        "本版本仅 test 编译；没有新运行时观测、星级写入或第二套生效门禁。RAW、GPU、重新检测与完整桌面流程未验收。",
        "0.1/0.2 和历史标签原图均保留。不得原地改本版参数遮掩失败；正式替换门禁前仍需要来源交叉、人物/拍摄组隔离的独立留出，每类至少 20 张。", ""]
    return "\n".join(lines)
