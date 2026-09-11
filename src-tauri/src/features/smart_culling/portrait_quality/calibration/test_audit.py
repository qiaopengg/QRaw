import json
import unittest

from audit import freeze, verify
from labeler import ClarityState
from legacy import content_hash
import test_labeler


class AuditTests(unittest.TestCase):
    def setUp(self):
        fixture = test_labeler.ClarityLabelerTests()
        fixture.setUp()
        self.addCleanup(fixture.temp.cleanup)
        self.batch, self.state = fixture.batch, fixture.state
        self.state.set_label(fixture.digest, 1)
        item = self.state.items[0]
        intake = {"expected_count": 1, "inventory": [{"source_path": str(self.batch / "images" / "C001.png"), "sample_id": fixture.digest}]}
        observations = {"schema": "qraw-portrait-quality-probe-2", "exportPurpose": "clarityCalibration", "rows": [{
            "sha256": fixture.digest, "detections": [{"faceIndex": 0, "bbox": item["bbox"]}],
            "variants": [{"blurSigma": 0.0, "evidence": {"inputSource": "decodedRgb8", "inputDimensions": [30, 30]}}]}]}
        manifest = json.loads((self.batch / "manifest.json").read_text())
        for name, key, payload in [("intake.json", "intake_sha256", intake), ("observations.json", "observations_sha256", observations)]:
            (self.batch / name).write_text(json.dumps(payload))
            manifest[key] = content_hash(self.batch / name)
        (self.batch / "manifest.json").write_text(json.dumps(manifest))
        payload = json.loads(self.state.json_path.read_text())
        payload["manifest_sha256"] = content_hash(self.batch / "manifest.json")
        self.state.json_path.write_text(json.dumps(payload))

    def test_read_only_complete_verification_preserves_all_files(self):
        before = {path: content_hash(path) for path in self.batch.rglob("*") if path.is_file()}
        result = verify(self.batch, expected_count=1)
        self.assertTrue(result["completion_passed"])
        self.assertFalse(result["accuracy_validated"])
        self.assertEqual(result["distribution"], {"清晰": 1})
        self.assertTrue(all(content_hash(path) == digest for path, digest in before.items()))
        with self.assertRaises(ValueError):
            ClarityState(self.batch, read_only=True).set_label(self.state.items[0]["sample_id"], 2)

    def test_csv_mismatch_is_not_repaired_by_audit(self):
        self.state.csv_path.write_text("wrong csv")
        before = self.state.csv_path.read_bytes()
        with self.assertRaises(ValueError):
            verify(self.batch, expected_count=1)
        self.assertEqual(before, self.state.csv_path.read_bytes())

    def test_incomplete_batch_stops_before_analysis(self):
        payload = json.loads(self.state.json_path.read_text())
        payload["labels"] = []
        self.state.json_path.write_text(json.dumps(payload))
        with self.assertRaises(ValueError):
            verify(self.batch, expected_count=1)

    def test_default_count_still_requires_the_original_24_samples(self):
        with self.assertRaises(ValueError):
            verify(self.batch)
        for count in [0, -1, True, 1.0]:
            with self.subTest(count=count), self.assertRaises(ValueError):
                verify(self.batch, expected_count=count)

    def test_explicit_count_cannot_override_the_prelabel_intake(self):
        intake = json.loads((self.batch / "intake.json").read_text())
        intake["expected_count"] = 2
        (self.batch / "intake.json").write_text(json.dumps(intake))
        manifest = json.loads((self.batch / "manifest.json").read_text())
        manifest["intake_sha256"] = content_hash(self.batch / "intake.json")
        (self.batch / "manifest.json").write_text(json.dumps(manifest))
        labels = json.loads(self.state.json_path.read_text())
        labels["manifest_sha256"] = content_hash(self.batch / "manifest.json")
        self.state.json_path.write_text(json.dumps(labels))
        with self.assertRaisesRegex(ValueError, "事前"):
            verify(self.batch, expected_count=1)

    def test_nonstandard_batch_cannot_inherit_the_old_data_version(self):
        with self.assertRaises(ValueError):
            freeze(self.batch, expected_count=1)
        self.assertFalse((self.batch / "冻结版本_v1.0").exists())

    def test_explicit_freeze_preserves_originals_and_rejects_overwrite(self):
        target = self.batch / "冻结版本_v1.0"

        def unlock_snapshot():
            if target.exists():
                target.chmod(0o700)
                for path in target.rglob("*"):
                    if not path.is_symlink():
                        path.chmod(0o700 if path.is_dir() else 0o600)

        self.addCleanup(unlock_snapshot)
        original = self.batch / "images" / "C001.png"
        before = original.read_bytes(), original.stat().st_mode
        result = freeze(self.batch, expected_count=1, data_version="qraw-clarity-synthetic-test-v1.0")
        self.assertEqual(result["data_version"], "qraw-clarity-synthetic-test-v1.0")
        self.assertEqual(verify(target, expected_count=1)["file_hashes"], result["file_hashes"])
        self.assertEqual((original.read_bytes(), original.stat().st_mode), before)
        with self.assertRaises(FileExistsError):
            freeze(self.batch, expected_count=1, data_version="qraw-clarity-synthetic-test-v1.0")


if __name__ == "__main__":
    unittest.main()
