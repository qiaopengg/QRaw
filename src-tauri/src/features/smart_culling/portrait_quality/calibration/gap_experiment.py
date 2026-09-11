"""Shared create-new workflow for fixed offline gap candidates 0.3 and 0.4."""

from pathlib import Path

from candidate import metrics
from legacy import content_hash
from prepare import write_new
from robust_io import read_json


def fit_baseline(output, reader, classifier):
    if (output / "candidate.json").exists():
        raise ValueError("已拟合或拒绝过此版本；禁止再次拟合")
    protocol, rows = reader(output)
    if protocol["phase"] != "baseline":
        raise ValueError("禁止从抗扰结果拟合")
    anchors = [dict(row["variants"][0], label=row["label"], sample_id=row["sample_id"],
                    image_ref=row["image_ref"]) for row in rows]
    failure = None
    try:
        candidate = classifier.fit(anchors)
    except ValueError as error:
        failure = str(error)
        candidate = dict(version=classifier.VERSION, input_source=classifier.INPUT_SOURCE,
            measurement_version=classifier.METHOD["measurement_version"], method=classifier.METHOD["method"],
            fit_status="rejected", failure_reason=failure, thresholds={},
            runtime_enabled=False, release_ready=False, independent_validation=False,
            calibration_count=len(anchors))
    measured = metrics(anchors, [classifier.predict(row, candidate) for row in anchors])
    ranges = {name: {str(label): [dict(image_ref=row["image_ref"], value=classifier.detail_row(row)["detail"].get(name))
                                for row in anchors if row["label"] == label] for label in (1, 2)} for name in classifier.FEATURES}
    write_new(output / "candidate.json", candidate)
    write_new(output / "diagnosis.json", dict(candidate_parameters=candidate, baseline=measured,
        feature_values=ranges, observations_sha256=content_hash(output / "observations.json"),
        protocol_sha256=content_hash(output / "PROTOCOL.json"), rows=anchors,
        source_confounded=True, independent_holdout_count=0, runtime_enabled=False, release_ready=False))
    with (output / "DIAGNOSIS_REPORT.md").open("x", encoding="utf-8") as target:
        target.write(f"# {classifier.VERSION}：原图一次拟合\n\n状态：{candidate['fit_status']}。"
            f"{failure or '阈值已固定；还不是抗扰或独立留出验收。'}\n\n"
            f"原图同批正确 {measured['correct']}/{measured['total']}，模糊误放 {measured['blurry_false_pass']}，"
            f"清晰误弃 {measured['clear_false_discard']}，未知 {measured['unknown']}。\n\n"
            "若拟合拒绝，以上全未知只是空门槛占位，不存在有效分类器。"
            "来源完全混杂，独立留出 0；所有原图保留、不删除困难侧脸或更改特征。\n")
    return dict(fit_status=candidate["fit_status"], failure=failure, baseline=measured)


def prepare_stress(baseline, output, reader, prepare):
    protocol, _ = reader(baseline)
    candidate = read_json(baseline / "candidate.json")
    diagnosis = read_json(baseline / "diagnosis.json")
    if (protocol["phase"] != "baseline" or candidate != diagnosis["candidate_parameters"]
            or diagnosis["observations_sha256"] != content_hash(baseline / "observations.json")
            or diagnosis["protocol_sha256"] != content_hash(baseline / "PROTOCOL.json")):
        raise ValueError("拟合记录或原图观测已改变")
    prepare(Path(protocol["frozen_path"]), output, "stress", baseline_dir=str(baseline),
        baseline_observations_sha256=content_hash(baseline / "observations.json"),
        candidate_sha256=content_hash(baseline / "candidate.json"))
    write_new(output / "candidate.json", candidate)
    return candidate


def read_stress_pair(output, reader, evidence_key):
    from stress_summary import same_detail
    protocol, rows = reader(output)
    if protocol["phase"] != "stress":
        raise ValueError("需要完整抗扰实验")
    baseline_dir = Path(protocol["baseline_dir"])
    if (content_hash(output / "candidate.json") != protocol["candidate_sha256"]
            or content_hash(baseline_dir / "observations.json") != protocol["baseline_observations_sha256"]):
        raise ValueError("固定参数或基线变化")
    _, baseline = reader(baseline_dir)
    lookup = {row["sample_id"]: row["variants"][0] for row in baseline}
    for row in rows:
        if not same_detail(row["variants"][0][evidence_key], lookup[row["sample_id"]][evidence_key], 1e-8):
            raise ValueError("抗扰基线复测不同")
    return protocol, rows, read_json(output / "candidate.json")
