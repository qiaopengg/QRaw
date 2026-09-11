import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from diagnose import analyze


class DiagnoseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.batch = Path(self.temp.name)
        (self.batch / "labels").mkdir()
        self.completion = dict(file_hashes={"observations.json": "frozen-observations"})
        self.snapshot = dict(file_hashes=self.completion["file_hashes"], code_hashes={"gate": "version-1"},
                             data_version="test-frozen")
        items, labels, observed, replayed = [], [], [], []
        for index, label in enumerate((1, 2)):
            sample = str(index)
            items.append(dict(sample_id=sample, image_ref=f"C{index}.png", face_index=0,
                              source_group_hint=f"source-{label}-A", bbox=[0, 0, 100, 100]))
            labels.append(dict(sample_id=sample, clarity_label=label))
            face = dict(faceIndex=0, qualityScore=0.9 if label == 1 else 0.2,
                        qualityUnavailableReason=None, cropDimensions=[100, 100],
                        detail=dict(nativeLaplacian=100 if label == 1 else 3,
                                    scaledLaplacian=400 if label == 1 else 30,
                                    blurRetentionRatio=0.1 if label == 1 else 0.5))
            observed.append(dict(sha256=sample, variants=[dict(evidence=dict(inputSource="decodedRgb8", faces=[face]))]))
            replayed.append(dict(sha256=sample, subjectFaceIndices=[0], state="uncertain"))
        self.replay = dict(schema="qraw-clarity-runtime-replay-1", usesActualRuntimeGate=True,
                           observationsSha256="frozen-observations", runtimeCodeHashes=self.snapshot["code_hashes"], rows=replayed)
        for name, payload in [("manifest.json", dict(items=items)), ("labels/clarity_labels.json", dict(labels=labels)),
                              ("observations.json", dict(rows=observed)), ("FREEZE_REPORT.json", self.snapshot),
                              ("replay.json", self.replay)]:
            self.write(name, payload)
        verification = patch("diagnose.verify", return_value=self.completion)
        verification.start()
        self.addCleanup(verification.stop)

    def write(self, name, payload):
        (self.batch / name).write_text(json.dumps(payload))

    def analyze(self):
        return analyze(self.batch, self.batch / "replay.json")

    def test_diagnosis_keeps_in_sample_results_out_of_runtime(self):
        result = self.analyze()
        self.assertEqual(result["legacy"]["unknown"], 2)
        self.assertEqual(result["candidate"]["correct"], 2)
        self.assertEqual(result["data_version"], "test-frozen")
        self.assertTrue(result["source_confounded"])
        self.assertFalse(result["release_ready"] or result["runtime_changed"])
        self.assertEqual(result["independent_holdout_count"], 0)

    def test_changed_freeze_is_not_silently_refitted(self):
        self.write("FREEZE_REPORT.json", dict(self.snapshot, file_hashes={}))
        with self.assertRaisesRegex(ValueError, "冻结文件已变化"):
            self.analyze()

    def test_unmatched_runtime_or_observations_are_rejected(self):
        for key, value in [("runtimeCodeHashes", {}), ("observationsSha256", "different")]:
            with self.subTest(key=key):
                self.write("replay.json", dict(self.replay, **{key: value}))
                with self.assertRaisesRegex(ValueError, "回放来源不一致"):
                    self.analyze()

    def test_wrong_subject_or_duplicate_sample_is_not_mixed_into_accuracy(self):
        self.replay["rows"][0]["subjectFaceIndices"] = [1]
        self.write("replay.json", self.replay)
        with self.assertRaisesRegex(ValueError, "人工指定脸与生效主体不一致"):
            self.analyze()
        self.replay["rows"].append(self.replay["rows"][0])
        self.write("replay.json", self.replay)
        with self.assertRaisesRegex(ValueError, "回放样本不完整或重复"):
            self.analyze()


if __name__ == "__main__":
    unittest.main()
