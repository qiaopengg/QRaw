import copy
import json
import unittest

from audit import verify
from legacy import content_hash
from paired_degradation import OBSERVATIONS_SCHEMA, checked_read, matrix, prepare
from paired_degradation_report import analyze, drift, vectors
from prepare import write_new
import test_audit


class PairedDegradationTests(unittest.TestCase):
    def test_matrix_keeps_orders_seeds_and_matching_clean_references(self):
        cases = matrix()
        self.assertEqual(len(cases), 206)
        self.assertEqual(len({case["id"] for case in cases}), 206)
        clean = {case["id"] for case in cases if case["id"] == case["reference_id"]}
        self.assertEqual(len(clean), 8)
        self.assertTrue(all(case["reference_id"] in clean for case in cases))
        noisy = [case for case in cases if case["id"] not in clean]
        self.assertEqual(len(noisy), 198)
        self.assertEqual({case["seed"] for case in noisy}, {0x51455741, 1, 20260910})
        for case in cases:
            self.assertEqual(case["reference_id"].endswith("_sharpen"),
                             bool(case["stages"] and case["stages"][-1]["kind"] == "unsharpen"))
            if case["id"].endswith("_before_blur"):
                self.assertIn(case["stages"][0]["kind"], ("noise", "shared_noise"))
                self.assertEqual(case["stages"][-1]["kind"], "blur")

    def test_incomplete_reordered_or_modified_experiment_is_rejected(self):
        fixture = test_audit.AuditTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        batch = fixture.batch
        complete = verify(batch, expected_count=1)
        write_new(batch / "FREEZE_REPORT.json", dict(complete, data_version="isolated-test"))
        output = batch / "experiment"
        output.mkdir()
        (output / "DESIGN.md").write_text("fixed isolated test design")
        protocol = prepare(batch, output, expected_count=1)
        observed = dict(schema=OBSERVATIONS_SCHEMA,
            protocol_sha256=content_hash(output / "PROTOCOL.json"), code_hashes=protocol["code_hashes"],
            rows=[dict(protocol["samples"][0], variants=[dict(case_id=case["id"]) for case in matrix()])])
        path = output / "observations.json"
        path.write_text(json.dumps(observed))
        self.assertEqual(len(checked_read(output)[1][0]["variants"]), 206)
        with self.assertRaises(FileExistsError):
            prepare(batch, output, expected_count=1)
        for change in ("missing", "duplicate", "sample"):
            broken = copy.deepcopy(observed)
            if change == "missing":
                broken["rows"][0]["variants"].pop()
            elif change == "duplicate":
                broken["rows"][0]["variants"][1] = broken["rows"][0]["variants"][0]
            else:
                broken["rows"][0]["sample_id"] = "wrong"
            path.write_text(json.dumps(broken))
            with self.assertRaises(ValueError):
                checked_read(output)
        path.write_text(json.dumps(observed))
        (output / "DESIGN.md").write_text("changed design")
        with self.assertRaises(ValueError):
            checked_read(output)

    def test_sharpened_signal_is_compared_to_sharpened_reference_and_no_accuracy_is_invented(self):
        cases = matrix()
        variants = []
        for case in cases:
            energy = 1000.0 if case["reference_id"].endswith("_sharpen") else 10.0
            variants.append(dict(case_id=case["id"], cross_channel=dict(scales=[dict(commonEnergy=energy)] * 2)))
        result = analyze(dict(cases=cases), [dict(sample_id="test", image_ref="test.png", variants=variants)])
        self.assertEqual(result["groups"]["noise"]["cross"]["stable_fraction"], 1.0)
        self.assertEqual(result["groups"]["shared_noise"]["cross"]["stable_fraction"], 1.0)
        self.assertFalse(result["classification_evaluable"])
        self.assertFalse(result["candidate_fitted"])
        self.assertFalse(result["release_ready"])
        self.assertEqual(drift([-5.0], [5.0]), 2.0)
        self.assertIsNone(drift([], []))
        self.assertIsNone(drift([None], [1.0]))
        self.assertIsNone(drift([float("nan")], [1.0]))
        self.assertEqual(vectors(dict(detail=dict(nativeLaplacian=10, scaledLaplacian=4)))["raw"], [10, 4])


if __name__ == "__main__":
    unittest.main()
