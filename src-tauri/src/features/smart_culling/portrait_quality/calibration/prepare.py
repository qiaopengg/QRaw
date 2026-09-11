"""Prepare an immutable, known-data clarity pilot; never edit source images or star labels."""

import argparse
import json
import math
from collections import defaultdict
from pathlib import Path

from PIL import Image

from legacy import TOOL_PATH, content_hash, utc_now

PROTOCOL = Path(__file__).parents[2] / "docs" / "PORTRAIT_CLARITY_CALIBRATION.md"


def write_new(path, payload):
    with path.open("x", encoding="utf-8") as output:
        json.dump(payload, output, ensure_ascii=False, indent=2, allow_nan=False)
        output.write("\n")


def select(output, sources):
    groups = defaultdict(list)
    inventory = []
    for source in sources:
        for path in sorted(source.resolve(strict=True).iterdir()):
            if path.suffix.lower() not in {".jpg", ".jpeg", ".png"}:
                continue
            digest = content_hash(path)
            with Image.open(path) as image:
                image.load()
                if image.getexif().get(274, 1) != 1:
                    raise ValueError(f"需先统一解码方向，不能静默排除：{path}")
                dimensions = list(image.size)
            prefix = path.stem.split("_")[0] if "_" in path.stem else "ungrouped"
            group = f"source-{sources.index(source) + 1}-{prefix}"
            entry = dict(source_path=str(path), sample_id=digest, dimensions=dimensions,
                         source_group_hint=group, provenance="unverified", split="calibration")
            inventory.append(entry)
            groups[group].append(entry)
    # One content-hash-selected image per existing filename group, without reading ratings.
    selected, seen = [], set()
    for group in sorted(groups):
        for entry in sorted(groups[group], key=lambda item: item["sample_id"]):
            if entry["sample_id"] not in seen:
                selected.append(entry)
                seen.add(entry["sample_id"])
                break
    if len(selected) != 24:
        raise ValueError(f"本轮预设 24 张，实际 {len(selected)}；先核查，不静默截断")
    output.mkdir(parents=True, exist_ok=False)
    (output / "images").mkdir()
    for index, entry in enumerate(sorted(selected, key=lambda item: item["sample_id"]), 1):
        path = Path(entry["source_path"])
        entry["image_ref"] = f"C{index:03d}{path.suffix.lower()}"
        (output / "images" / entry["image_ref"]).symlink_to(path)
    # Protocol is frozen BEFORE human labels exist. It is not a production threshold policy.
    with (output / "PROTOCOL.md").open("xb") as target:
        target.write(PROTOCOL.read_bytes())
    write_new(output / "intake.json", dict(schema="qraw-clarity-intake-1", created_at=utc_now(),
        protocol_sha256=content_hash(output / "PROTOCOL.md"), base_tool_sha256=content_hash(TOOL_PATH),
        selection="one lowest unseen SHA-256 per filename group", inventory=inventory, items=selected))
    print(f"已准备 {len(selected)} 张只读匿名图片；候选盘点 {len(inventory)} 张：{output}")


def finalize(batch):
    intake = json.loads((batch / "intake.json").read_text())
    report_path = batch / "observations.json"
    report = json.loads(report_path.read_text())
    if report["schema"] != "qraw-portrait-quality-probe-2" or report["exportPurpose"] != "clarityCalibration":
        raise ValueError("不是当前清晰度原图观测导出")
    rows = {row["sha256"]: row for row in report["rows"]}
    if len(rows) != len(intake["items"]) or len(rows) != len(report["rows"]):
        raise ValueError("观测数量不一致或重复")
    items = []
    for entry in sorted(intake["items"], key=lambda item: item["image_ref"]):
        if content_hash(Path(entry["source_path"])) != entry["sample_id"]:
            raise ValueError("源文件已改变")
        row = rows[entry["sample_id"]]
        if [variant["blurSigma"] for variant in row["variants"]] != [0.0]:
            raise ValueError("人工只看原图，不混入自动模糊变体")
        evidence = row["variants"][0]["evidence"]
        if evidence["inputDimensions"] != entry["dimensions"] or evidence["inputSource"] != "decodedRgb8":
            raise ValueError("显示与测量像素契约不一致")
        width, height = entry["dimensions"]
        valid = []
        for face in row["detections"]:
            x, y, w, h = face["bbox"]
            if (all(math.isfinite(v) for v in (x, y, w, h)) and w > 0 and h > 0
                    and x >= 0 and y >= 0 and x + w <= width and y + h <= height):
                valid.append(face)
        # Explicit annotation target, NOT a new runtime subject-selection or clarity gate.
        face = max(valid, key=lambda item: item["bbox"][2] * item["bbox"][3], default=None)
        items.append(dict(entry, face_index=face["faceIndex"] if face else None,
                          bbox=face["bbox"] if face else None))
    write_new(batch / "manifest.json", dict(schema="qraw-clarity-batch-1", batch_name="人脸清晰度 · 第001批",
        created_at=utc_now(), protocol_sha256=intake["protocol_sha256"],
        base_tool_sha256=intake["base_tool_sha256"], observations_sha256=content_hash(report_path),
        intake_sha256=content_hash(batch / "intake.json"), release_ready=False,
        independent_holdout=False, identity_groups_verified=False, items=items))
    print(f"已冻结 {len(items)} 张标注清单，未建立任何人工答案；无有效脸框 {sum(i['bbox'] is None for i in items)} 张")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["select", "finalize"])
    parser.add_argument("--batch", required=True, type=Path)
    parser.add_argument("--source", action="append", type=Path, default=[])
    args = parser.parse_args()
    if not args.batch.is_absolute():
        parser.error("批次目录必须是绝对路径")
    select(args.batch, args.source) if args.action == "select" else finalize(args.batch)
