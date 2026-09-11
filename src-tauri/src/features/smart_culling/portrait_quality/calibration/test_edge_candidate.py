import copy
import math
import unittest

from edge_candidate import INPUT_SOURCE, METHOD, fit, predict


def row(label, width):
    return dict(label=label, bbox=[10, 10, 120, 120], input_source=INPUT_SOURCE,
                edge_spread=dict(version=METHOD["measurement_version"], acceptedProfiles=20, widthP25=width))


class EdgeCandidateTests(unittest.TestCase):
    def test_fixed_direction_and_source_protection(self):
        rows = [row(1, 3), row(1, 4), row(2, 9), row(2, 12)]
        candidate = fit(rows)
        self.assertEqual([predict(r, candidate) for r in rows], ["clear", "clear", "unclear", "unclear"])
        self.assertEqual(predict(row(1, 6.5), candidate), "uncertain")
        self.assertEqual(predict(dict(rows[0], input_source="renderedRgb8"), candidate), "uncertain")
        self.assertFalse(candidate["runtime_enabled"])

    def test_missing_or_insufficient_support_is_not_blur(self):
        candidate = fit([row(1, 3), row(2, 10)])
        for patch in [dict(widthP25=None), dict(widthP25=math.nan), dict(widthP25=0),
                      dict(acceptedProfiles=5), dict(acceptedProfiles=True), dict(version="unknown")]:
            r = row(1, 3)
            r["edge_spread"].update(patch)
            self.assertEqual(predict(r, candidate), "uncertain")
            with self.assertRaises(ValueError):
                fit([r, row(2, 10)])

    def test_overlap_is_rejected_and_other_candidate_cannot_be_reused(self):
        with self.assertRaises(ValueError):
            fit([row(1, 9), row(2, 8)])
        candidate = fit([row(1, 3), row(2, 10)])
        for key, value in [("version", "old"), ("fit_status", "rejected"), ("measurement_version", "old")]:
            changed = copy.deepcopy(candidate)
            changed[key] = value
            self.assertEqual(predict(row(1, 3), changed), "uncertain")
