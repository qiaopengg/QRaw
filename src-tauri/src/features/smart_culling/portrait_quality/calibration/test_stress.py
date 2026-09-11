import copy
import json
import unittest
from pathlib import Path

from candidate import fit
from stress_summary import summarize
from test_candidate import row


class StressSummaryTests(unittest.TestCase):
    def setUp(self):
        self.protocol = json.loads(Path(__file__).with_name("stress_protocol.json").read_text())
        self.protocol["sample_count"] = 2
        anchors = [row(1, 100, 400, 0.1), row(2, 3, 30, 0.5)]
        self.candidate = fit(anchors)
        self.rows = []
        for index, anchor in enumerate(anchors):
            variants = [dict(case_id=case["id"], input_source="decodedRgb8" if case["kind"] == "original" else "controlledStressRgb8",
                             bbox=anchor["bbox"], detail=copy.deepcopy(anchor["detail"])) for case in self.protocol["cases"]]
            self.rows.append(dict(sample_id=str(index), image_ref=f"C{index}.jpg", label=anchor["label"],
                                  frozen_detail=copy.deepcopy(anchor["detail"]), variants=variants))

    def summarize(self):
        return summarize(self.rows, self.protocol, self.candidate)

    def test_guarded_and_counterfactual_results_cannot_be_confused(self):
        result = self.summarize()
        self.assertTrue(result["screening_passed"])
        self.assertFalse(result["release_ready"] or result["synthetic_accuracy_claimed"])
        self.assertEqual(result["rendered_source_accepted"], 0)
        variant = result["rows"][0]["variants"][1]
        self.assertEqual(variant["source_guarded_state"], "uncertain")
        self.assertEqual(variant["numeric_projection"], "clear")

    def test_noise_promotion_fails_without_hiding_the_sample(self):
        noisy = next(v for v in self.rows[1]["variants"] if v["case_id"] == "noise_24")
        noisy["detail"] = copy.deepcopy(self.rows[0]["frozen_detail"])
        result = self.summarize()
        self.assertFalse(result["screening_passed"])
        self.assertEqual(result["noise_sharpen_promotions"], {"noise_24": ["C1.jpg"]})
        self.assertEqual(result["by_case"]["noise_24"]["total"], 2)

    def test_background_pixel_leak_fails_even_when_class_does_not_change(self):
        self.rows[0]["variants"][1]["detail"]["nativeLaplacian"] += 1
        result = self.summarize()
        self.assertFalse(result["screening_passed"])
        self.assertEqual(result["by_case"]["background_flat"]["changed"], 0)
        self.assertEqual(result["background_detail_changes"], 1)

    def test_missing_or_duplicate_samples_and_cases_are_rejected(self):
        for invalid in [self.rows[:1], [self.rows[0], self.rows[0]]]:
            with self.assertRaises(ValueError):
                summarize(invalid, self.protocol, self.candidate)
        self.rows[0]["variants"].pop()
        with self.assertRaises(ValueError):
            self.summarize()

    def test_changed_baseline_stops_the_report(self):
        self.rows[0]["variants"][0]["detail"]["nativeLaplacian"] += 1
        with self.assertRaisesRegex(ValueError, "原图复测与冻结观测不同"):
            self.summarize()


if __name__ == "__main__":
    unittest.main()
