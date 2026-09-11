import copy
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import test_robust_candidate
from linear_candidate import METHOD, fit, predict
from linear_experiment import fit_baseline
from linear_report import summarize


class LinearCandidateTests(unittest.TestCase):
    def setUp(self):
        fixture = test_robust_candidate.RobustCandidateTests()
        fixture.setUp()
        self.rows, self.protocol = fixture.rows, fixture.protocol
        for row in self.rows:
            fine, ratio = (100.0, 0.25) if row["label"] == 1 else (3.0, 0.8)
            for variant in row["variants"]:
                variant["linear_detail"] = dict(version=METHOD["measurement_version"], noiseSigma=1.0,
                    scales=[dict(smoothingOrder=2, correctedEnergy=fine, observedEnergy=fine + 1),
                            dict(smoothingOrder=4, correctedEnergy=fine * ratio)], coarseToFineRatio=ratio)
        self.anchors = [dict(row["variants"][0], label=row["label"]) for row in self.rows]
        self.candidate = fit(self.anchors)

    def test_native_version_and_positive_correction_are_required(self):
        self.assertEqual([predict(row, self.candidate) for row in self.anchors], ["clear", "unclear"])
        self.assertFalse(self.candidate["runtime_enabled"] or self.candidate["release_ready"])
        for replacement in [{"input_source": "renderedRgb8"}, {"unavailable_reason": "partialFace"}]:
            self.assertEqual(predict(dict(self.anchors[0], **replacement), self.candidate), "uncertain")
        for field, value in [("version", "old"), ("coarseToFineRatio", None), ("noiseSigma", float("nan"))]:
            row = copy.deepcopy(self.anchors[0])
            row["linear_detail"][field] = value
            self.assertEqual(predict(row, self.candidate), "uncertain")
        row = copy.deepcopy(self.anchors[1])
        row["linear_detail"]["scales"][0]["correctedEnergy"] = -1
        self.assertEqual(predict(row, self.candidate), "uncertain")
        with self.assertRaises(ValueError):
            fit([dict(row, input_source="renderedRgb8") for row in self.anchors])

    def test_overlap_is_rejected_and_saved_without_refitting(self):
        self.rows[0]["variants"][0]["linear_detail"]["coarseToFineRatio"] = 0.9
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for name in ["PROTOCOL.json", "observations.json"]:
                (directory / name).write_text("{}")
            with patch("linear_experiment.read", return_value=({"phase": "baseline"}, self.rows)):
                self.assertFalse(fit_baseline(directory))
                with self.assertRaisesRegex(ValueError, "禁止再次拟合"):
                    fit_baseline(directory)
            candidate = json.loads((directory / "candidate.json").read_text())
            self.assertEqual(candidate["fit_status"], "rejected")
            self.assertEqual(candidate["thresholds"], {})
            self.assertIn("coarseToFineRatio", candidate["failure_reason"])

    def test_report_rejects_abstention_and_fit_failure_even_with_zero_promotions(self):
        self.assertTrue(summarize(self.rows, self.protocol, self.candidate)["screening_passed"])
        for row in self.rows:
            next(v for v in row["variants"] if v["case_id"] == "noise_24")["linear_detail"] = None
        result = summarize(self.rows, self.protocol, self.candidate)
        self.assertFalse(result["screening_passed"])
        self.assertEqual(result["noise_coverage_checks"]["noise_24"]["unknown"], 2)
        self.assertEqual(result["numerical_ranges_by_original_label"]["noise_24"]["1"]["fine_corrected"]["missing"], 1)
        result = summarize(self.rows, self.protocol, dict(self.candidate, fit_status="rejected", thresholds={}))
        self.assertFalse(result["classifier_valid"] or result["screening_passed"])
        self.assertEqual(result["baseline"]["unknown"], 2)

    def test_promotions_discards_and_background_changes_are_all_reported(self):
        for index, row in enumerate(self.rows):
            next(v for v in row["variants"] if v["case_id"] == "noise_24")["linear_detail"] = copy.deepcopy(
                self.anchors[1 - index]["linear_detail"])
        self.rows[0]["variants"][1]["linear_detail"]["noiseSigma"] += 1
        result = summarize(self.rows, self.protocol, self.candidate)
        self.assertFalse(result["screening_passed"])
        self.assertEqual(result["noise_sharpen_promotions"], {"noise_24": ["C1.jpg"]})
        self.assertEqual(result["noise_coverage_checks"]["noise_24"]["clear_to_unclear"], 1)
        self.assertEqual(result["background_detail_changes"], 1)


if __name__ == "__main__":
    unittest.main()
