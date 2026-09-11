import csv
import http.client
import json
import tempfile
import threading
import unittest
from pathlib import Path
from unittest.mock import patch

from PIL import Image

from labeler import ClarityState, make_server
from legacy import TOOL_PATH, content_hash
from prepare import write_new


class ClarityLabelerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.batch = Path(self.temp.name)
        (self.batch / "images").mkdir()
        path = self.batch / "images" / "C001.png"
        Image.new("RGB", (30, 30), "gray").save(path)
        self.digest = content_hash(path)
        manifest = dict(schema="qraw-clarity-batch-1", batch_name="isolated test",
                        base_tool_sha256=content_hash(TOOL_PATH))
        for name, key in [("PROTOCOL.md", "protocol_sha256"), ("intake.json", "intake_sha256"),
                          ("observations.json", "observations_sha256")]:
            write_new(self.batch / name, {})
            manifest[key] = content_hash(self.batch / name)
        manifest["items"] = [dict(sample_id=self.digest, image_ref=path.name, source_path=str(path),
                                  face_index=0, bbox=[5, 5, 20, 20], dimensions=[30, 30])]
        write_new(self.batch / "manifest.json", manifest)
        self.state = ClarityState(self.batch)

    def test_clarity_schema_and_resume_are_separate_from_expression(self):
        self.state.set_label(self.digest, 2)
        resumed = ClarityState(self.batch)
        self.assertEqual(resumed.public_state()["completed"], 1)
        self.assertEqual(resumed.public_state()["items"][0]["label"], 2)
        payload = json.loads(resumed.json_path.read_text())
        self.assertEqual(payload["labels"][0]["clarity_state"], "明显模糊")
        self.assertNotIn("expression_label", resumed.json_path.read_text())
        self.assertFalse((resumed.output / "expression_labels.json").exists())
        with resumed.csv_path.open(encoding="utf-8-sig", newline="") as source:
            rows = list(csv.DictReader(source))
        self.assertEqual(rows[0]["clarity_label"], "2")
        resumed.set_label(self.digest, None)
        self.assertEqual(ClarityState(self.batch).public_state()["completed"], 0)
        self.assertEqual(content_hash(self.batch / "images" / "C001.png"), self.digest)

    def test_invalid_and_unknown_labels_rejected(self):
        for label in [0, 4, 5, True, "1", 1.5]:
            with self.subTest(label=label), self.assertRaises(ValueError):
                self.state.set_label(self.digest, label)
        with self.assertRaises(ValueError):
            self.state.set_label("not-a-sample", 1)
        self.assertEqual(self.state.public_state()["completed"], 0)

    def test_missing_face_only_allows_explicit_skip(self):
        self.state.items[0]["bbox"] = None
        with self.assertRaises(ValueError):
            self.state.set_label(self.digest, 2)
        self.assertEqual(self.state.set_label(self.digest, 3)["completed"], 1)

    def test_freeze_mismatch_and_changed_image_fail_loudly(self):
        with (self.batch / "PROTOCOL.md").open("a") as output:
            output.write("changed")
        with self.assertRaises(ValueError):
            ClarityState(self.batch)
        Image.new("RGB", (30, 30), "red").save(self.batch / "images" / "C001.png")
        with self.assertRaises(ValueError):
            self.state.set_label(self.digest, 1)

    def test_failed_save_does_not_update_memory_or_canonical_json(self):
        before = self.state.json_path.read_bytes()
        with patch("labeler.atomic_write_text", side_effect=OSError("disk full")):
            with self.assertRaises(OSError):
                self.state.set_label(self.digest, 1)
        self.assertEqual(self.state.public_state()["completed"], 0)
        self.assertEqual(self.state.json_path.read_bytes(), before)

    def test_public_state_does_not_leak_observations_or_mutable_state(self):
        public = self.state.public_state()
        public["items"][0]["label"] = 2
        self.assertIsNone(self.state.items[0]["label"])
        for key in ("source_path", "quality_score", "expression_label", "split"):
            self.assertNotIn(key, json.dumps(public))

    def test_http_guard_and_round_trip(self):
        server = make_server(self.state, 0)
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)

        def request(method, route, payload=None, origin=None):
            connection = http.client.HTTPConnection("127.0.0.1", server.server_port)
            headers = {"Content-Type": "application/json"}
            if origin:
                headers["Origin"] = origin
            connection.request(method, route, json.dumps(payload) if payload is not None else None, headers)
            response = connection.getresponse()
            status, body = response.status, response.read()
            connection.close()
            return status, body

        origin = "http://" + server.expected_host
        self.assertEqual(request("GET", "/api/state")[0], 200)
        self.assertEqual(request("GET", "/image/" + self.digest)[0], 200)
        self.assertEqual(request("GET", "/image/../../manifest.json")[0], 404)
        self.assertEqual(request("POST", "/api/label", {}, "https://external.invalid")[0], 403)
        self.assertEqual(request("POST", "/api/label", {}, None)[0], 403)
        for payload in [[], {"sample_id": self.digest, "label": True}, {"sample_id": self.digest, "label": 5}]:
            self.assertEqual(request("POST", "/api/label", payload, origin)[0], 400)
        status, data = request("POST", "/api/label", {"sample_id": self.digest, "label": 1}, origin)
        self.assertEqual(status, 200)
        self.assertEqual(json.loads(data)["completed"], 1)
        self.assertEqual(ClarityState(self.batch).public_state()["completed"], 1)
        status, _ = request("POST", "/api/clear", {"sample_id": self.digest}, origin)
        self.assertEqual(status, 200)
        self.assertEqual(self.state.public_state()["completed"], 0)


if __name__ == "__main__":
    unittest.main()
