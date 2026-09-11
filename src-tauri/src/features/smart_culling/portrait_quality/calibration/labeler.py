"""Clarity-only adapter for the existing local annotation transport. No star writes."""

import argparse
import copy
import csv
import io
import json
import threading
from http import HTTPStatus
from http.server import ThreadingHTTPServer
from pathlib import Path

from legacy import TOOL_PATH, atomic_write_text, content_hash, tool, utc_now

LABELS = {1: "清晰", 2: "明显模糊", 3: "暂不判断"}
SCHEMA = "qraw-clarity-labels-1"
FIELDS = ["sample_id", "image_ref", "face_index", "clarity_label", "clarity_state", "updated_at"]


class ClarityState:
    def __init__(self, batch, *, read_only=False):
        self.read_only = read_only
        self.batch = batch.resolve(strict=True)
        self.manifest_hash = content_hash(self.batch / "manifest.json")
        manifest = json.loads((self.batch / "manifest.json").read_text())
        if manifest["schema"] != "qraw-clarity-batch-1":
            raise ValueError("标注清单版本错误")
        for name, key in [("PROTOCOL.md", "protocol_sha256"), ("observations.json", "observations_sha256"),
                          ("intake.json", "intake_sha256")]:
            if content_hash(self.batch / name) != manifest[key]:
                raise ValueError(f"冻结内容已改变：{name}")
        if content_hash(TOOL_PATH) != manifest["base_tool_sha256"]:
            raise ValueError("既有标注工具已改变，需先复核")
        self.batch_name = manifest["batch_name"]
        self.lock = threading.Lock()
        self.allowed_labels = tuple(LABELS)
        self.items, self.paths_by_id = [], {}
        for entry in manifest["items"]:
            path = self.batch / "images" / entry["image_ref"]
            sample = entry["sample_id"]
            if path.resolve(strict=True) != Path(entry["source_path"]).resolve(strict=True) or content_hash(path) != sample:
                raise ValueError("匿名图片与原图不一致")
            if sample in self.paths_by_id:
                raise ValueError("重复样本")
            self.paths_by_id[sample] = path
            # No model scores, historic labels, source paths or split hints sent to the UI.
            self.items.append({**{key: entry[key] for key in
                ("sample_id", "image_ref", "face_index", "bbox", "dimensions")}, "label": None})
        if not self.items:
            raise ValueError("空批次")
        self.output = self.batch / "labels"
        if not read_only:
            self.output.mkdir(exist_ok=True)
        self.json_path = self.output / "clarity_labels.json"
        self.csv_path = self.output / "clarity_labels.csv"
        self.labels = {}
        if self.json_path.exists():
            payload = json.loads(self.json_path.read_text())
            if payload.get("schema") != SCHEMA or payload.get("manifest_sha256") != self.manifest_hash:
                raise ValueError("标注结果属于不同任务或清单")
            for record in payload["labels"]:
                item = next((i for i in self.items if i["sample_id"] == record["sample_id"]), None)
                label = record["clarity_label"]
                if (item is None or type(label) is not int or label not in LABELS
                        or record["sample_id"] in self.labels or record["face_index"] != item["face_index"]
                        or record["image_ref"] != item["image_ref"] or record["clarity_state"] != LABELS[label]):
                    raise ValueError("非法或重复的清晰度标签")
                item["label"] = label
                self.labels[item["sample_id"]] = record
        # JSON is the canonical auto-save; CSV is a derived export regenerated on resume.
        if not read_only:
            self._persist(self.labels)

    def public_state(self):
        with self.lock:
            return self._state()

    def _state(self):
        return dict(schema=SCHEMA, batch_name=self.batch_name, total=len(self.items),
                    completed=len(self.labels), items=copy.deepcopy(self.items))

    def set_label(self, sample, label):
        if self.read_only:
            raise ValueError("只读核查不能修改标签")
        with self.lock:
            item = next((item for item in self.items if item["sample_id"] == sample), None)
            if item is None:
                raise ValueError("未知样本")
            if label is not None and (type(label) is not int or label not in LABELS):
                raise ValueError("本轮只允许清晰、明显模糊、暂不判断；不是星级")
            if label in (1, 2) and item["bbox"] is None:
                raise ValueError("没有指定人脸，请选暂不判断")
            if content_hash(self.paths_by_id[sample]) != sample:
                raise ValueError("照片内容已经变化，停止保存")
            labels = dict(self.labels)
            if label is None:
                labels.pop(sample, None)
            else:
                labels[sample] = dict(sample_id=sample, image_ref=item["image_ref"],
                    face_index=item["face_index"], clarity_label=label, clarity_state=LABELS[label], updated_at=utc_now())
            self._persist(labels)
            self.labels, item["label"] = labels, label
            return self._state()

    def _persist(self, labels):
        records = [labels[item["sample_id"]] for item in self.items if item["sample_id"] in labels]
        payload = dict(schema=SCHEMA, manifest_sha256=self.manifest_hash, labels=records)
        stream = io.StringIO(newline="")
        writer = csv.DictWriter(stream, fieldnames=FIELDS)
        writer.writeheader()
        writer.writerows(records)
        # Commit JSON last. A failed request never publishes uncommitted labels in memory.
        atomic_write_text(self.csv_path, "\ufeff" + stream.getvalue())
        atomic_write_text(self.json_path, json.dumps(payload, ensure_ascii=False, indent=2) + "\n")


class ClarityHandler(tool.AnnotationHandler):
    server_version = "QRawClarityLabeler/1.0"

    def do_GET(self):
        if self.headers.get("Host") != self.app.expected_host:
            self.send_error(HTTPStatus.FORBIDDEN)
            return
        super().do_GET()

    def do_POST(self):
        if (self.headers.get("Host") != self.app.expected_host
                or self.headers.get("Origin") != "http://" + self.app.expected_host
                or self.headers.get("Content-Type") != "application/json"):
            self.send_error(HTTPStatus.FORBIDDEN)
            return
        if self.path not in {"/api/label", "/api/clear"}:
            self.send_error(HTTPStatus.NOT_FOUND)
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            if not 0 < length <= 4096:
                raise ValueError("请求大小无效")
            payload = json.loads(self.rfile.read(length))
            if not isinstance(payload, dict) or not isinstance(payload.get("sample_id"), str):
                raise ValueError("请求样本无效")
            label = None if self.path == "/api/clear" else payload.get("label")
            if self.path == "/api/label" and (type(label) is not int or label not in LABELS):
                raise ValueError("本轮只允许 1、2、3 三个按钮，不是星级")
            self._send_json(self.app.state.set_label(payload["sample_id"], label))
        except (ValueError, KeyError) as error:
            self._send_json({"error": str(error)}, HTTPStatus.BAD_REQUEST)
        except OSError:
            self._send_json({"error": "磁盘保存失败，请不要继续，联系 AI 检查"}, HTTPStatus.INTERNAL_SERVER_ERROR)


def make_server(state, port):
    server = ThreadingHTTPServer(("127.0.0.1", port), ClarityHandler)
    server.state = state
    server.ui_path = Path(__file__).with_name("index.html")
    server.expected_host = f"127.0.0.1:{server.server_port}"
    return server


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--batch", required=True, type=Path)
    parser.add_argument("--port", type=int, default=8766)
    args = parser.parse_args()
    with make_server(ClarityState(args.batch), args.port) as server:
        print(f"清晰度标注已就绪：http://{server.expected_host}", flush=True)
        try:
            server.serve_forever(poll_interval=0.25)
        except KeyboardInterrupt:
            pass
