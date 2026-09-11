"""Versioned inputs for 0.2; reuses frozen-batch audit and the original stress matrix."""

import json
from pathlib import Path

from legacy import content_hash, utc_now
from prepare import write_new
from robust_candidate import VERSION
from stress import FEATURE, MEASUREMENT_FILES, checked_frozen
from stress_summary import same_detail

CODE_FILES = list(dict.fromkeys([*MEASUREMENT_FILES, "portrait_quality/robust_detail.rs",
    "portrait_quality/clarity_observation.rs", "infrastructure/clarity_input.rs",
    "../../image_loader.rs", "../../image_processing.rs",
    *[f"portrait_quality/calibration/{name}.py" for name in
      ["candidate", "robust_candidate", "robust_io", "robust_experiment", "robust_report", "stress_summary"]]]))


def read_json(path):
    return json.loads(path.read_text())


def prepare_protocol(frozen, output, phase, *, candidate_version=VERSION,
                     measurement="robustDetailV1", code_files=CODE_FILES, notes=None,
                     protocol_version="qraw-clarity-robust-stress-2", **extra):
    snapshot = checked_frozen(frozen)
    original = read_json(Path(__file__).with_name("stress_protocol.json"))
    checks = dict(original["checks"], require_calibration_floor=True,
                  noise_unknown_rate_max=0.20, noise_clear_to_unclear_allowed=0)
    protocol = dict(original, version=protocol_version, candidate_version=candidate_version,
        measurement=measurement, phase=phase, checks=checks,
        notes=notes if notes is not None else ["0.2 只在原图上拟合一次；随后参数固定，复用 0.1 全部 16 条件。",
               "noiseSigma 只观测，不事后选择噪声阈值；5x5 中值滤波只是待检验假设。",
               "合成条件不新增人工真值；额外报告清晰降档及未知，不能全弃权报成功。"],
        created_at=utc_now(), frozen_path=str(frozen), frozen_file_hashes=snapshot["file_hashes"],
        measurement_code_hashes={name: content_hash(FEATURE / name) for name in code_files},
        matrix_sha256=content_hash(Path(__file__).with_name("stress_protocol.json")), **extra)
    if phase == "baseline":
        protocol["cases"] = original["cases"][:1]
    output.mkdir(exist_ok=False)
    write_new(output / "PROTOCOL.json", protocol)
    return protocol


def read_experiment(output, *, candidate_version=VERSION,
                    observation_schema="qraw-clarity-stress-observations-2"):
    protocol = read_json(output / "PROTOCOL.json")
    if protocol["candidate_version"] != candidate_version or protocol["phase"] not in {"baseline", "stress"}:
        raise ValueError(f"不是 {candidate_version} 协议")
    frozen = Path(protocol["frozen_path"])
    snapshot = checked_frozen(frozen)
    if snapshot["file_hashes"] != protocol["frozen_file_hashes"]:
        raise ValueError("冻结输入变化")
    for name, digest in protocol["measurement_code_hashes"].items():
        if content_hash(FEATURE / name) != digest:
            raise ValueError(f"实验代码变化：{name}")
    observed = read_json(output / "observations.json")
    if (observed["schema"] != observation_schema
            or observed["protocol_sha256"] != content_hash(output / "PROTOCOL.json")
            or observed["measurement_code_hashes"] != protocol["measurement_code_hashes"]):
        raise ValueError("观测与协议不对应")
    items = {item["sample_id"]: item for item in read_json(frozen / "manifest.json")["items"]}
    labels = {row["sample_id"]: row["clarity_label"] for row in read_json(frozen / "labels/clarity_labels.json")["labels"]}
    old = {row["sha256"]: row for row in read_json(frozen / "observations.json")["rows"]}
    rows = observed["rows"]
    if len(rows) != len(items) or {row["sample_id"] for row in rows} != set(items):
        raise ValueError("样本缺失或重复")
    for row in rows:
        item = items[row["sample_id"]]
        variants = row["variants"]
        if [v["case_id"] for v in variants] != [case["id"] for case in protocol["cases"]]:
            raise ValueError("实验矩阵不完整")
        base = variants[0]
        if base["bbox"] != item["bbox"] or base["input_dimensions"] != item["dimensions"] or row["image_ref"] != item["image_ref"]:
            raise ValueError("原图指定脸或坐标不一致")
        detail = next(f["detail"] for f in old[row["sample_id"]]["variants"][0]["evidence"]["faces"]
                      if f["faceIndex"] == item["face_index"])
        if not same_detail(detail, base["detail"], protocol["detail_absolute_tolerance"]):
            raise ValueError("原图细节与历史观测不同")
        row.update(label=labels[row["sample_id"]], frozen_detail=detail)
    return protocol, rows
