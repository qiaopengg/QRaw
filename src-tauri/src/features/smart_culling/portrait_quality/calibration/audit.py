"""Read-only completion checks and create-new snapshots; never repair data while auditing."""

import argparse
import csv
import json
import shutil
from collections import Counter
from pathlib import Path

from labeler import ClarityState, FIELDS
from legacy import content_hash, utc_now
from prepare import write_new

EXPECTED_COUNT = 24
DEFAULT_DATA_VERSION = "qraw-clarity-batch-001-v1.0"
SNAPSHOT_FILES = ["manifest.json", "PROTOCOL.md", "intake.json", "observations.json",
                  "labels/clarity_labels.json", "labels/clarity_labels.csv"]


def verify(batch, *, expected_count=EXPECTED_COUNT):
    if type(expected_count) is not int or expected_count <= 0:
        raise ValueError("事前样本数必须是正整数")
    state = ClarityState(batch, read_only=True)
    public = state.public_state()
    if public["total"] != expected_count or public["completed"] != expected_count:
        raise ValueError(f"未全部完成或事前数量不符：{public['completed']}/{public['total']}，预设 {expected_count}")
    with state.csv_path.open(encoding="utf-8-sig", newline="") as source:
        reader = csv.DictReader(source)
        if reader.fieldnames != FIELDS:
            raise ValueError("CSV 字段不一致")
        csv_rows = list(reader)
    json_rows = [state.labels[item["sample_id"]] for item in state.items]
    expected = [{key: "" if row[key] is None else str(row[key]) for key in FIELDS} for row in json_rows]
    if csv_rows != expected:
        raise ValueError("JSON/CSV 不一致；保留现场，不自动覆盖修复")
    intake = json.loads((batch / "intake.json").read_text())
    declared_count = intake.get("expected_count", expected_count)
    if type(declared_count) is not int or declared_count != expected_count:
        raise ValueError("事前 intake 样本数与审计要求不一致")
    for item in intake["inventory"]:
        if content_hash(Path(item["source_path"])) != item["sample_id"]:
            raise ValueError(f"候选原图已改变：{item['source_path']}")
    report = json.loads((batch / "observations.json").read_text())
    rows = {row["sha256"]: row for row in report["rows"]}
    if (report["schema"] != "qraw-portrait-quality-probe-2" or report["exportPurpose"] != "clarityCalibration"
            or len(rows) != expected_count or len(rows) != len(report["rows"])
            or set(rows) != set(state.labels)):
        raise ValueError("观测版本、数量、内容映射不一致")
    for item in state.items:
        row = rows[item["sample_id"]]
        if [v["blurSigma"] for v in row["variants"]] != [0.0]:
            raise ValueError("不是单一原图观测")
        evidence = row["variants"][0]["evidence"]
        if evidence["inputSource"] != "decodedRgb8" or evidence["inputDimensions"] != item["dimensions"]:
            raise ValueError("像素契约不一致")
        face = next((f for f in row["detections"] if f["faceIndex"] == item["face_index"]), None)
        if item["bbox"] is not None and (face is None or face["bbox"] != item["bbox"]):
            raise ValueError("指定人脸与测量对象不一致")
    return dict(schema="qraw-clarity-completion-1", total=public["total"], completed=public["completed"],
        distribution=dict(Counter(row["clarity_state"] for row in json_rows)),
        original_files_verified=len(intake["inventory"]),
        file_hashes={name: content_hash(batch / name) for name in SNAPSHOT_FILES},
        completion_passed=True, accuracy_validated=False)


def freeze(batch, *, expected_count=EXPECTED_COUNT, data_version=DEFAULT_DATA_VERSION):
    if (not isinstance(data_version, str) or not data_version.strip()
            or (expected_count != EXPECTED_COUNT and data_version == DEFAULT_DATA_VERSION)):
        raise ValueError("新批次必须显式指定独立数据版本，不能沿用旧 24 张版本")
    summary = verify(batch, expected_count=expected_count)
    target = batch / "冻结版本_v1.0"
    target.mkdir(exist_ok=False)
    for name in SNAPSHOT_FILES:
        destination = target / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(batch / name, destination)
        if content_hash(destination) != summary["file_hashes"][name]:
            raise ValueError("冻结副本与原结果不同")
    (target / "images").mkdir()
    for image in json.loads((batch / "manifest.json").read_text())["items"]:
        (target / "images" / image["image_ref"]).symlink_to(Path(image["source_path"]))
    verify(target, expected_count=expected_count)
    # Capture the exact business code used for later Rust replay; do not copy identity data.
    feature = Path(__file__).parents[2]
    sources = ["mode_evidence.rs", "mode_scoring.rs", "analysis.rs", "portrait_quality.rs",
               "portrait_quality/detail.rs", "portrait_quality/face.rs", "portrait_quality/optical.rs",
               "portrait_quality/model.rs", "portrait_quality/calibration_export.rs"]
    summary.update(data_version=data_version, expected_count=expected_count, frozen_at=utc_now(),
                   audit_source_sha256=content_hash(Path(__file__)),
                   code_hashes={name: content_hash(feature / name) for name in sources})
    write_new(target / "FREEZE_REPORT.json", summary)
    # Only copied snapshot files/directories; never chmod a symlink or an original photo.
    for path in target.rglob("*"):
        if not path.is_symlink():
            path.chmod(0o555 if path.is_dir() else 0o444)
    target.chmod(0o555)
    return dict(summary, frozen_path=str(target))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--batch", required=True, type=Path)
    parser.add_argument("--freeze", action="store_true")
    parser.add_argument("--expected-count", type=int, default=EXPECTED_COUNT)
    parser.add_argument("--data-version", default=DEFAULT_DATA_VERSION)
    args = parser.parse_args()
    if not args.batch.is_absolute():
        parser.error("必须使用绝对批次路径")
    result = (freeze(args.batch, expected_count=args.expected_count, data_version=args.data_version)
              if args.freeze else verify(args.batch, expected_count=args.expected_count))
    print(json.dumps(result, ensure_ascii=False, indent=2))
