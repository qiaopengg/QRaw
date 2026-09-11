import copy
import unittest

from robust_candidate import INPUT_SOURCE, MEASUREMENT_VERSION, fit, predict
from robust_report import summarize_robust
import test_stress


class RobustCandidateTests(unittest.TestCase):
    def setUp(self):
        fixture = test_stress.StressSummaryTests()
        fixture.setUp()
        self.rows, self.protocol = fixture.rows, fixture.protocol
        self.protocol["checks"].update(noise_unknown_rate_max=0.20, noise_clear_to_unclear_allowed=0)
        for row in self.rows:
            for variant in row["variants"]:
                if variant["case_id"] == "original":
                    variant["input_source"] = INPUT_SOURCE
                variant["robust_detail"] = dict(version=MEASUREMENT_VERSION, medianRadius=2,
                    noiseSigma=1.0, denoisedDetail=copy.deepcopy(variant["detail"]))
        self.anchors = [dict(row["variants"][0], label=row["label"]) for row in self.rows]
        self.candidate = fit(self.anchors)

    def test_only_versioned_native_denoised_evidence_drives_the_new_candidate(self):
        self.assertEqual([predict(row, self.candidate) for row in self.anchors], ["clear", "unclear"])
        self.assertFalse(self.candidate["runtime_enabled"] or self.candidate["release_ready"])
        changed = dict(self.anchors[1], detail=self.anchors[0]["detail"], fiqa=1, source_hint="clear")
        self.assertEqual(predict(changed, self.candidate), "unclear")
        for field, value in [("version", "old"), ("medianRadius", 1), ("noiseSigma", float("nan"))]:
            changed = copy.deepcopy(self.anchors[0])
            changed["robust_detail"][field] = value
            self.assertEqual(predict(changed, self.candidate), "uncertain")
        self.assertEqual(predict(dict(self.anchors[0], input_source="renderedRgb8"), self.candidate), "uncertain")
        with self.assertRaises(ValueError):
            fit([dict(row, input_source="renderedRgb8") for row in self.anchors])

    def test_all_unknown_noise_cannot_be_reported_as_success(self):
        for row in self.rows:
            next(v for v in row["variants"] if v["case_id"] == "noise_24")["robust_detail"] = None
        result = summarize_robust(self.rows, self.protocol, self.candidate)
        self.assertFalse(result["screening_passed"])
        self.assertEqual(result["noise_coverage_checks"]["noise_24"]["unknown"], 2)
        self.assertEqual(result["noise_sharpen_promotions"], {})

    def test_promotions_and_false_discards_are_both_visible(self):
        blurry = next(v for v in self.rows[1]["variants"] if v["case_id"] == "noise_24")
        blurry["robust_detail"] = copy.deepcopy(self.anchors[0]["robust_detail"])
        clear = next(v for v in self.rows[0]["variants"] if v["case_id"] == "noise_24")
        clear["robust_detail"] = copy.deepcopy(self.anchors[1]["robust_detail"])
        result = summarize_robust(self.rows, self.protocol, self.candidate)
        self.assertFalse(result["screening_passed"])
        self.assertEqual(result["noise_sharpen_promotions"], {"noise_24": ["C1.jpg"]})
        self.assertEqual(result["noise_coverage_checks"]["noise_24"]["clear_to_unclear"], 1)

    def test_robust_background_leak_is_detected_even_if_raw_detail_is_identical(self):
        result = summarize_robust(self.rows, self.protocol, self.candidate)
        self.assertTrue(result["screening_passed"])
        self.rows[0]["variants"][1]["robust_detail"]["noiseSigma"] += 1
        result = summarize_robust(self.rows, self.protocol, self.candidate)
        self.assertFalse(result["screening_passed"])
        self.assertEqual(result["background_detail_changes"], 1)


if __name__ == "__main__":
    unittest.main()
