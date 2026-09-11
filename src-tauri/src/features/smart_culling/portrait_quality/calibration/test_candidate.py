import copy
import unittest

from candidate import best_fiqa_threshold, fit, metrics, predict


def row(label, native, scaled, retention, fiqa=None):
    return dict(label=label, input_source="decodedRgb8", bbox=[1, 2, 100, 100], fiqa=fiqa,
                detail=dict(nativeLaplacian=native, scaledLaplacian=scaled, blurRetentionRatio=retention))


class CandidateTests(unittest.TestCase):
    def setUp(self):
        self.rows = [row(1, 100, 400, 0.10), row(1, 120, 500, 0.08),
                     row(2, 3, 30, 0.50), row(2, 4, 50, 0.45)]
        self.candidate = fit(self.rows)

    def test_candidate_is_offline_and_uses_gap_not_filename_or_fiqa(self):
        self.assertFalse(self.candidate["runtime_enabled"])
        self.assertFalse(self.candidate["release_ready"])
        predictions = [predict(item, self.candidate) for item in self.rows]
        self.assertEqual(predictions, ["clear", "clear", "unclear", "unclear"])
        for fiqa in (-100, 100, None):
            changed = dict(self.rows[0], fiqa=fiqa, source_hint="misleading", expression_label=1)
            self.assertEqual(predict(changed, self.candidate), "clear")

    def test_missing_conflicting_or_incompatible_evidence_abstains(self):
        cases = [row(1, 100, 30, 0.10), row(1, 50, 200, 0.28),
                 dict(self.rows[0], input_source="renderedRgb8"), dict(self.rows[0], bbox=None),
                 dict(self.rows[0], bbox=[0, 0, 0, 0]), row(1, float("nan"), 400, 0.1),
                 row(1, -1, 400, 0.1), dict(self.rows[0], detail={})]
        for item in cases:
            with self.subTest(item=item):
                self.assertEqual(predict(item, self.candidate), "uncertain")
        malformed = copy.deepcopy(self.candidate)
        malformed["thresholds"] = {}
        self.assertEqual(predict(self.rows[0], malformed), "uncertain")

    def test_overlap_or_missing_class_is_not_forced_into_a_model(self):
        for rows in [self.rows[:2], [self.rows[0], row(2, 200, 500, 0.5)]]:
            with self.assertRaises(ValueError):
                fit(rows)

    def test_unknown_is_in_accuracy_denominator_and_skip_is_separate(self):
        rows = [*self.rows, row(3, 10, 100, 0.2)]
        result = metrics(rows, ["clear", "uncertain", "unclear", "clear", "uncertain"])
        self.assertEqual((result["correct"], result["total"], result["unknown"], result["human_skipped"]), (2, 4, 1, 1))
        self.assertEqual(result["blurry_false_pass_rate"], 0.5)
        self.assertFalse(result["calibration_floor_passed"])
        self.assertIsNone(metrics([], [])["accuracy"])
        self.assertFalse(metrics(self.rows[:2], ["clear", "clear"])["calibration_floor_passed"])

    def test_best_fiqa_threshold_does_not_drop_missing_scores(self):
        rows = [row(1, 100, 400, 0.1, 0.9), row(1, 100, 400, 0.1, None), row(2, 4, 30, 0.5, 0.2)]
        result = best_fiqa_threshold(rows)
        self.assertEqual(result["metrics"]["correct"], 2)
        self.assertEqual(result["metrics"]["total"], 3)
        self.assertEqual(result["metrics"]["unknown"], 1)


if __name__ == "__main__":
    unittest.main()
