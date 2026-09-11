"""Analyze frozen human truth with runtime replay and explicitly in-sample candidates."""

import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path

from audit import verify
from candidate import FEATURES, best_fiqa_threshold, fit, metrics, predict
from legacy import content_hash, utc_now
from prepare import write_new


def analyze(frozen, replay_path):
    completion = verify(frozen)
    snapshot = json.loads((frozen / "FREEZE_REPORT.json").read_text())
    if snapshot["file_hashes"] != completion["file_hashes"]:
        raise ValueError("冻结文件已变化；保留现场，不重新拟合")
    manifest = json.loads((frozen / "manifest.json").read_text())
    labels = {row["sample_id"]: row for row in json.loads((frozen / "labels/clarity_labels.json").read_text())["labels"]}
    observed = {row["sha256"]: row for row in json.loads((frozen / "observations.json").read_text())["rows"]}
    replay = json.loads(replay_path.read_text())
    if (replay["schema"] != "qraw-clarity-runtime-replay-1" or not replay["usesActualRuntimeGate"]
            or replay["observationsSha256"] != completion["file_hashes"]["observations.json"]
            or replay["runtimeCodeHashes"] != snapshot["code_hashes"]):
        raise ValueError("旧门禁回放来源不一致")
    old = {row["sha256"]: row for row in replay["rows"]}
    if set(old) != set(labels) or len(old) != len(replay["rows"]):
        raise ValueError("回放样本不完整或重复")
    rows = []
    for item in manifest["items"]:
        sample = item["sample_id"]
        observation = observed[sample]
        evidence = observation["variants"][0]["evidence"]
        face = next((face for face in evidence["faces"] if face["faceIndex"] == item["face_index"]), None)
        if face is None or old[sample]["subjectFaceIndices"] != [item["face_index"]]:
            raise ValueError("人工指定脸与生效主体不一致，必须单独排查，不混算")
        rows.append(dict(sample_id=sample, image_ref=item["image_ref"], label=labels[sample]["clarity_label"],
            source_hint=item["source_group_hint"], input_source=evidence["inputSource"], bbox=item["bbox"],
            detail=face["detail"] or {}, fiqa=face["qualityScore"], fiqa_unavailable=face["qualityUnavailableReason"],
            crop_dimensions=face["cropDimensions"], old_state=old[sample]["state"]))
    candidate = fit(rows)
    for row in rows:
        row["candidate_state"] = predict(row, candidate)
    by_source = defaultdict(Counter)
    for row in rows:
        by_source[row["source_hint"].rsplit("-", 1)[0]][str(row["label"])] += 1
    ranges = {feature: {str(label): {"min": min(r["detail"][feature] for r in rows if r["label"] == label),
                                    "max": max(r["detail"][feature] for r in rows if r["label"] == label)}
                       for label in (1, 2)} for feature in FEATURES}
    report = dict(schema="qraw-clarity-diagnosis-1", created_at=utc_now(),
        data_version=snapshot["data_version"], frozen_path=str(frozen), completion=completion,
        replay_sha256=content_hash(replay_path), candidate_source_sha256=content_hash(Path(__file__).with_name("candidate.py")),
        legacy=metrics(rows, [row["old_state"] for row in rows]),
        candidate=metrics(rows, [row["candidate_state"] for row in rows]),
        fiqa_single_threshold=best_fiqa_threshold(rows), feature_ranges=ranges,
        source_label_counts=dict(by_source), rows=rows, candidate_parameters=candidate,
        candidate_fit_on_all_labels=True, independent_holdout_count=0, runtime_changed=False,
        source_confounded=all(len(counts) == 1 for counts in by_source.values()),
        unvalidated_scenarios=["清晰背景/模糊脸与相反组合", "不同来源的交叉清晰/模糊样本", "小脸", "暗光", "噪声", "美颜/锐化", "RAW/编辑后预览"],
        release_ready=False)
    return report


def markdown(report):
    old, new = report["legacy"], report["candidate"]
    fiqa = report["fiqa_single_threshold"]["metrics"]
    completion = report["completion"]
    counts = completion["distribution"]
    lines = ["# 清晰度第 001 批：冻结与诊断结果", "",
        f"{counts.get('清晰', 0)} 张清晰、{counts.get('明显模糊', 0)} 张明显模糊，{completion['completed']}/{completion['total']} 完成并通过数据完整性检查。原图和正式评分未修改。", "",
        "## 结果", "", "| 方法 | 正确 / 全部明确答案 | 清晰误弃 | 模糊误放 | 无法确定 |", "| --- | --- | --- | --- | --- |"]
    for name, result in [("现有实际门禁回放", old), ("eDifFIQA 单阈值的同数据最优诊断", fiqa), ("局部细节候选 0.1（同数据拟合）", new)]:
        lines.append(f"| {name} | {result['correct']}/{result['total']} | {result['clear_false_discard']} | {result['blurry_false_pass']} | {result['unknown']} |")
    lines += ["", "现有门禁识别 13/14 张清晰照片，但 10/10 张明显模糊照片都停在无法确定。剩余 1 张清晰侧脸也无法确定；其检测分约 0.735、原人脸锐度约 76.62，未同时达到旧清晰通过线。", "",
        "10 张模糊照片的整图锐度约 2.53–3.91、人脸锐度约 2.99–5.08，均未满足旧规则要求的整图 <2.4 且人脸 <2.8。问题不是没有取得人脸指标，而是旧明确模糊门槛过窄且依赖整图。", "",
        "eDifFIQA 分数在清晰和模糊之间重叠；C024 清晰侧脸因 strongProfile 没有模型分数。缺失不能作为模糊，质量总分也不能独立替代局部清晰度。", "",
        "## 候选 0.1（不启用正式评分）", "", "候选只使用原尺度/受控尺度拉普拉斯与模糊细节保留比，不读取来源、表情或眼态。三个相关指标必须一致，冲突、缺失或不是 decodedRgb8 时弃权。每项在两类已知极值间留出中间一半作为不确定区。这是看过本批答案后的校准规则，不是盲测。", "",
        "| 指标 | 模糊侧门槛 | 清晰侧门槛 | 方向 |", "| --- | --- | --- | --- |"]
    for name, bounds in report["candidate_parameters"]["thresholds"].items():
        lines.append(f"| {name} | {bounds['blurry']:.6f} | {bounds['clear']:.6f} | {'越高越清晰' if bounds['direction'] == 1 else '越低越清晰'} |")
    lines += ["", "## 为什么不能直接称为可用", "", f"候选在本批拟合 {new['correct']}/{new['total']} 正确，不是独立验证成绩。10 张模糊恰好全部来自来源 1，14 张清晰来自来源 2/3；即使未使用来源字段，这种来源与答案完全关联也会高估泛化能力。没有独立留出、没有经核验的人物/拍摄组隔离。", "",
        "本批指定脸短边最小约 167 像素，没有覆盖小脸；暗光、噪声、美颜等也没有场景真值。强侧脸 1 张只算已观察个案，不算侧脸群体验收。14/10 都不足以替代预设的独立留出每类至少 20 张。", "",
        "人工首轮已结束，不重标这 24 张。下一步应先做新候选的抗干扰与预览尺度检查，再准备不同来源、不同人物/拍摄组的留出验证；不能把本批剩余相似照片随机划成独立盲测。现有唯一生效门禁和最终星级契约保持不变。", "",
        "## 复现与追溯", "", "冻结数据见本目录上级的 冻结版本_v1.0；数字化逐张结果、原门禁回放哈希及候选代码哈希见 diagnosis.json，参数见 candidate.json。所有输出为新版本，未覆盖原始标签。", ""]
    return "\n".join(lines)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--frozen", required=True, type=Path)
    parser.add_argument("--replay", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    report = analyze(args.frozen, args.replay)
    args.output.mkdir(exist_ok=False)
    write_new(args.output / "diagnosis.json", report)
    write_new(args.output / "candidate.json", report["candidate_parameters"])
    with (args.output / "DIAGNOSIS_REPORT.md").open("x", encoding="utf-8") as output:
        output.write(markdown(report))
    print(json.dumps({key: report[key] for key in ["legacy", "candidate", "source_label_counts", "release_ready"]}, ensure_ascii=False, indent=2))
