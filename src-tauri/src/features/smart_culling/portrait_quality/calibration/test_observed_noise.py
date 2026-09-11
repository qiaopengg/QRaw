import copy
import json
import unittest

import noise_decomposition as decomposition
import observed_noise as experiment
import test_noise_decomposition
from legacy import content_hash
from observed_noise_checks import verify
from observed_noise_metrics import analyze
from observed_noise_report import report
from prepare import write_new
from stress import FEATURE


class ObservedNoiseTests(unittest.TestCase):
    def setUp(self):
        fixture = test_noise_decomposition.NoiseDecompositionTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        self.parent = fixture.output
        self.prior = copy.deepcopy(fixture.rows)
        for variant in self.prior[0]["variants"]:
            variant["cross_channel"] = dict(version="isolated", innerDimensions=[16, 16], sampleCount=100,
                scales=[dict(commonEnergy=10.0, smoothingOrder=order, channelEnergies=[10.0] * 3,
                             pairEnergies=[10.0] * 3) for order in (2, 4)], coarseToFineRatio=1.0)
            variant["decomposition"]["scales"] = [dict(copy.deepcopy(variant["decomposition"]["scales"][0]),
                order=order, count=100, identity_error=0.0) for order in (2, 4)]
        old_rows = [dict(self.prior[0], variants=[{key: v[key] for key in ("case_id", "crop_dimensions", "cross_channel")}
                                               for v in self.prior[0]["variants"]])]
        (fixture.parent / "observations.json").write_text(json.dumps(dict(fixture.parent_observed, rows=old_rows)))
        parent_protocol = decomposition.prepare(fixture.parent, self.parent)
        write_new(self.parent / "observations.json", dict(schema="qraw-noise-decomposition-observations-1",
            protocol_sha256=content_hash(self.parent / "PROTOCOL.json"), code_hashes=parent_protocol["code_hashes"], rows=self.prior))
        archive = self.parent / "measurement_code" / FEATURE.relative_to(FEATURE.parents[3])
        mapping = {}
        for name, digest in parent_protocol["code_hashes"].items():
            target = (archive / name).resolve()
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((FEATURE / name).read_bytes())
            mapping[name] = dict(path=str(target.relative_to(self.parent.resolve())), sha256=digest)
        write_new(self.parent / "CODE_MAP.json", mapping)
        for name in experiment.PARENT_FILES:
            if not (self.parent / name).exists():
                (self.parent / name).write_text("{}\n")
        self.output = fixture.output.parent / "observed-only"
        self.output.mkdir()
        (self.output / "DESIGN.md").write_text("isolated observed-only design")
        self.protocol = experiment.prepare(self.parent, self.output)
        self.rows = copy.deepcopy(self.prior)
        counts = [0] * 16
        for y in range(10):
            for x in range(10):
                counts[(y * 4 // 10) * 4 + x * 4 // 10] += 1
        for variant in self.rows[0]["variants"]:
            variant["estimate"] = dict(version=experiment.VERSION, input="observedRgb8Only", median_radius=2,
                block_grid=[4, 4], inner_dimensions=[16, 16], clean_reference_used=False, confidence_interval_validated=False,
                scales=[dict(proxy_decomposition=scale, corrected_energy=10.0,
                    sampling=dict(counts=counts, means=[0.0] * 16, delete_means=[0.0] * 16, mean=0.0, standard_error=0.0))
                    for scale in variant.pop("decomposition")["scales"]])
            variant.pop("trace")
        self.observed = dict(schema="qraw-observed-noise-observations-1", estimator_clean_reference_used=False,
            protocol_sha256=content_hash(self.output / "PROTOCOL.json"), code_hashes=self.protocol["code_hashes"], rows=self.rows)
        write_new(self.output / "observations.json", self.observed)

    def test_archived_parent_is_verified_and_outputs_cannot_be_replaced(self):
        self.assertEqual(experiment.checked_read(self.output)[3]["replay"], 0.0)
        with self.assertRaises(FileExistsError):
            experiment.prepare(self.parent, self.output)
        archive = self.parent / "measurement_code" / FEATURE.relative_to(FEATURE.parents[3])
        (archive / "portrait_quality/linear_response.rs").write_text("changed archived kernel")
        with self.assertRaises(ValueError):
            experiment.checked_read(self.output)

    def test_missing_cases_changed_failures_and_reference_input_are_rejected(self):
        path = self.output / "observations.json"
        for kind in ("missing", "reference"):
            broken = copy.deepcopy(self.observed)
            if kind == "missing":
                broken["rows"][0]["variants"].pop()
            else:
                broken["rows"][0]["variants"][0]["estimate"]["clean_reference_used"] = True
            path.write_text(json.dumps(broken))
            with self.assertRaises(ValueError):
                experiment.checked_read(self.output)
        path.write_text(json.dumps(self.observed))
        (self.parent / "CONCLUSIONS.md").write_text("changed prior conclusions")
        with self.assertRaises(ValueError):
            experiment.checked_read(self.output)

    def test_independent_checks_reject_numeric_and_block_tampering(self):
        for kind in ("pair", "spatial", "corrected", "blocks", "se", "nonfinite"):
            rows = copy.deepcopy(self.rows)
            scale = rows[0]["variants"][0]["estimate"]["scales"][0]
            if kind == "pair":
                scale["proxy_decomposition"]["pairs_rg_rb_gb"][0]["noise_energy"] = 100.0
            elif kind == "spatial":
                scale["proxy_decomposition"]["same_position"] = 100.0
            elif kind == "corrected":
                scale["corrected_energy"] = 100.0
            elif kind == "blocks":
                scale["sampling"]["counts"][0] += 1
            else:
                scale["sampling"]["standard_error"] = 100.0 if kind == "se" else float("nan")
            with self.assertRaises(ValueError, msg=kind):
                verify(self.protocol, rows, self.prior)

    def test_clean_leakage_cannot_be_hidden_by_good_noise_metrics(self):
        result = analyze(self.protocol, self.rows, self.prior)
        self.assertTrue(result["screening_passed"])
        self.assertEqual(len(result["seed_variation"]), 66)
        for flag in ("candidate_fitted", "classification_evaluable", "confidence_interval_validated", "release_ready"):
            self.assertFalse(result[flag])
        self.rows[0]["variants"][0]["estimate"]["scales"][0]["corrected_energy"] = -20.0
        result = analyze(self.protocol, self.rows, self.prior)
        self.assertFalse(result["clean_leakage_passed"])
        self.assertFalse(result["screening_passed"])
        self.assertEqual(result["groups"]["clean"]["negative_corrected"], 1)

    def test_full_report_preserves_diagnostic_boundary_and_refuses_overwrite(self):
        result = report(self.output)
        self.assertTrue(result["screening_passed"])
        self.assertFalse(result["classification_evaluable"])
        self.assertFalse(result["release_ready"])
        self.assertEqual(json.loads((self.output / "FAILURES.json").read_text()), [])
        digest = content_hash(self.output / "SUMMARY.json")
        with self.assertRaises(FileExistsError):
            report(self.output)
        self.assertEqual(content_hash(self.output / "SUMMARY.json"), digest)


if __name__ == "__main__":
    unittest.main()
