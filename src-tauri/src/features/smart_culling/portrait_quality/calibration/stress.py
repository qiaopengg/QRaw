"""Create-new frozen experiments and reports; no fitting, no source image writes."""

import argparse
import json
from pathlib import Path

from audit import verify
from legacy import content_hash, utc_now
from prepare import write_new
from stress_summary import markdown, summarize

FEATURE = Path(__file__).parents[2]
MEASUREMENT_FILES = ["analysis.rs", "portrait_quality/detail.rs", "portrait_quality/face.rs",
                     "portrait_quality/stress_probe.rs", "portrait_quality/stress_variants.rs"]


def checked_frozen(frozen):
    completion = verify(frozen)
    snapshot = json.loads((frozen / "FREEZE_REPORT.json").read_text())
    if completion["file_hashes"] != snapshot["file_hashes"]:
        raise ValueError("冻结文件已变化")
    return snapshot


def prepare(frozen, candidate_dir, output):
    snapshot = checked_frozen(frozen)
    config = json.loads(Path(__file__).with_name("stress_protocol.json").read_text())
    candidate = json.loads((candidate_dir / "candidate.json").read_text())
    diagnosis = json.loads((candidate_dir / "diagnosis.json").read_text())
    if (candidate["version"] != config["candidate_version"] or candidate != diagnosis["candidate_parameters"]
            or content_hash(Path(__file__).with_name("candidate.py")) != diagnosis["candidate_source_sha256"]
            or diagnosis["completion"]["file_hashes"] != snapshot["file_hashes"]):
        raise ValueError("不是原批次已冻结的候选 0.1；不重新拟合")
    hashes = {name: content_hash(FEATURE / name) for name in MEASUREMENT_FILES}
    for name, digest in hashes.items():
        if name in snapshot["code_hashes"] and digest != snapshot["code_hashes"][name]:
            raise ValueError(f"原测量代码已改变：{name}")
    output.mkdir(exist_ok=False)
    write_new(output / "candidate.json", candidate)
    write_new(output / "PROTOCOL.json", dict(config, created_at=utc_now(), frozen_path=str(frozen),
        data_version=snapshot["data_version"], frozen_file_hashes=snapshot["file_hashes"],
        candidate_sha256=content_hash(output / "candidate.json"), candidate_code_sha256=diagnosis["candidate_source_sha256"],
        measurement_code_hashes=hashes, diagnosis_sha256=content_hash(candidate_dir / "diagnosis.json"),
        application_input_code_hashes={name: content_hash(FEATURE / name) for name in ["runner.rs", "infrastructure/render_input.rs"]}))
    print(f"已固定 {config['sample_count']} 张 × {len(config['cases'])} 条件；未调用拟合：{output}")


def report(output):
    protocol = json.loads((output / "PROTOCOL.json").read_text())
    frozen = Path(protocol["frozen_path"])
    snapshot = checked_frozen(frozen)
    if (snapshot["file_hashes"] != protocol["frozen_file_hashes"]
            or content_hash(output / "candidate.json") != protocol["candidate_sha256"]
            or content_hash(Path(__file__).with_name("candidate.py")) != protocol["candidate_code_sha256"]):
        raise ValueError("实验数据或候选发生变化")
    observed = json.loads((output / "observations.json").read_text())
    if (observed["schema"] != "qraw-clarity-stress-observations-1"
            or observed["protocol_sha256"] != content_hash(output / "PROTOCOL.json")
            or observed["measurement_code_hashes"] != protocol["measurement_code_hashes"]):
        raise ValueError("观测不是本协议生成")
    manifest = json.loads((frozen / "manifest.json").read_text())
    labels = {row["sample_id"]: row["clarity_label"] for row in json.loads((frozen / "labels/clarity_labels.json").read_text())["labels"]}
    baseline = {row["sha256"]: row for row in json.loads((frozen / "observations.json").read_text())["rows"]}
    items = {row["sample_id"]: row for row in manifest["items"]}
    if set(row["sample_id"] for row in observed["rows"]) != set(items):
        raise ValueError("实验与冻结清单不同")
    rows = []
    for row in observed["rows"]:
        item = items[row["sample_id"]]
        original = row["variants"][0]
        if original["bbox"] != item["bbox"] or original["input_dimensions"] != item["dimensions"] or row["image_ref"] != item["image_ref"]:
            raise ValueError("原图对照坐标或尺寸不一致")
        faces = baseline[row["sample_id"]]["variants"][0]["evidence"]["faces"]
        detail = next(face["detail"] for face in faces if face["faceIndex"] == item["face_index"])
        rows.append(dict(row, label=labels[row["sample_id"]], frozen_detail=detail))
    candidate = json.loads((output / "candidate.json").read_text())
    result = summarize(rows, protocol, candidate)
    result.update(schema="qraw-clarity-stress-report-1", created_at=utc_now(),
                  protocol_sha256=content_hash(output / "PROTOCOL.json"), observations_sha256=content_hash(output / "observations.json"))
    write_new(output / "stress_report.json", result)
    with (output / "STRESS_REPORT.md").open("x", encoding="utf-8") as target:
        target.write(markdown(result))
    print(json.dumps({key: result[key] for key in ["baseline", "background_detail_changes", "noise_sharpen_promotions", "rendered_source_accepted", "screening_passed"]}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["prepare", "report"])
    parser.add_argument("--frozen", type=Path)
    parser.add_argument("--candidate-dir", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if any(path is not None and not path.is_absolute() for path in [args.frozen, args.candidate_dir, args.output]):
        parser.error("必须使用绝对路径")
    if args.action == "prepare":
        if not args.frozen or not args.candidate_dir:
            parser.error("准备实验需提供冻结批次及候选目录")
        prepare(args.frozen, args.candidate_dir, args.output)
    else:
        report(args.output)
