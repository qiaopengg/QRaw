import copy
import json
import unittest

import paired_degradation as paired
import noise_decomposition as experiment
import test_audit
from audit import verify
from decomposition_checks import TERM_NAMES, relative_error, verify_numerics
from decomposition_report import analyze
from legacy import content_hash
from prepare import write_new
from stress import FEATURE


class NoiseDecompositionTests(unittest.TestCase):
    def setUp(self):
        fixture = test_audit.AuditTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        batch = fixture.batch
        write_new(batch / "FREEZE_REPORT.json", dict(verify(batch, expected_count=1), data_version="isolated-test"))
        self.parent = batch / "paired"
        self.parent.mkdir()
        (self.parent / "DESIGN.md").write_text("isolated paired design")
        self.protocol = paired.prepare(batch, self.parent, expected_count=1)
        self.variants = []
        parent_variants = []
        for case in self.protocol["cases"]:
            energy = 10.0
            cross = dict(version="isolated", scales=[dict(commonEnergy=energy)] * 2)
            parent_variants.append(dict(case_id=case["id"], crop_dimensions=[80, 80], cross_channel=cross))
            pair = dict(signal_energy=energy, noise_energy=0.0, observed_energy=energy, cross_term=0.0, linearity_error=0.0)
            scale = dict(reference=energy, observed=energy, residual_common=0.0, signal_noise_cross=0.0,
                same_position=0.0, different_position=0.0, same_position_pairs=[0.0] * 3,
                different_position_pairs=[0.0] * 3, pairs_rg_rb_gb=[pair] * 3)
            self.variants.append(dict(parent_variants[-1], decomposition=dict(scales=[scale] * 2),
                trace=dict(reference=[energy] * 2, observed=[energy] * 2, actual_delta=[0.0] * 2,
                           terms={name: [0.0] * 2 for name in TERM_NAMES})))
        self.old_rows = [dict(self.protocol["samples"][0], variants=parent_variants)]
        self.rows = [dict(self.protocol["samples"][0], variants=self.variants)]
        self.parent_observed = dict(schema=paired.OBSERVATIONS_SCHEMA,
            protocol_sha256=content_hash(self.parent / "PROTOCOL.json"), code_hashes=self.protocol["code_hashes"], rows=self.old_rows)
        write_new(self.parent / "observations.json", self.parent_observed)
        self.archive = self.parent / "measurement_code" / FEATURE.relative_to(FEATURE.parents[3])
        mapping = {}
        for name, digest in self.protocol["code_hashes"].items():
            target = (self.archive / name).resolve()
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((FEATURE / name).read_bytes())
            mapping[name] = dict(path=str(target.relative_to(self.parent.resolve())), sha256=digest)
        write_new(self.parent / "CODE_MAP.json", mapping)
        for name in experiment.PARENT_FILES:
            if not (self.parent / name).exists():
                (self.parent / name).write_text("{}\n")
        self.output = batch / "decomposition"
        self.output.mkdir()
        (self.output / "DESIGN.md").write_text("isolated decomposition design")

    def test_old_protocol_and_archived_code_remain_verified(self):
        legacy = copy.deepcopy(self.protocol)
        legacy["schema"] = "qraw-paired-degradation-protocol-1"
        legacy["code_hashes"].pop("portrait_quality/paired_probe_io.rs")
        (self.parent / "PROTOCOL.json").write_text(json.dumps(legacy))
        observed = dict(self.parent_observed, schema="qraw-paired-degradation-observations-1",
            protocol_sha256=content_hash(self.parent / "PROTOCOL.json"), code_hashes=legacy["code_hashes"])
        (self.parent / "observations.json").write_text(json.dumps(observed))
        self.assertEqual(len(paired.checked_read(self.parent, code_root=self.archive)[1]), 1)
        path = self.archive / "portrait_quality/cross_channel_detail.rs"
        path.write_text("changed archive")
        with self.assertRaises(ValueError):
            paired.checked_read(self.parent, code_root=self.archive)

    def test_new_protocol_refuses_overwrite_missing_cases_and_changed_parent(self):
        protocol = experiment.prepare(self.parent, self.output)
        observed = dict(schema="qraw-noise-decomposition-observations-1", code_hashes=protocol["code_hashes"],
            protocol_sha256=content_hash(self.output / "PROTOCOL.json"), rows=self.rows)
        path = self.output / "observations.json"
        path.write_text(json.dumps(observed))
        self.assertEqual(experiment.checked_read(self.output)[2]["replay"], 0.0)
        with self.assertRaises(FileExistsError):
            experiment.prepare(self.parent, self.output)
        broken = copy.deepcopy(observed)
        broken["rows"][0]["variants"].pop()
        path.write_text(json.dumps(broken))
        with self.assertRaises(ValueError):
            experiment.checked_read(self.output)
        path.write_text(json.dumps(observed))
        (self.parent / "FAILURE_ANALYSIS.md").write_text("changed failure evidence")
        with self.assertRaises(ValueError):
            experiment.checked_read(self.output)

    def test_numerical_errors_fail_and_identity_success_never_becomes_accuracy(self):
        self.assertEqual(relative_error(-3.0, -3.0), 0.0)
        with self.assertRaises(ValueError):
            relative_error(float("nan"), 0.0)
        for kind in ("replay", "pair", "trace", "reference"):
            rows = copy.deepcopy(self.rows)
            variant = rows[0]["variants"][2]
            if kind == "replay":
                variant["cross_channel"]["scales"][0]["commonEnergy"] += 1.0
            elif kind == "pair":
                variant["decomposition"]["scales"][0]["pairs_rg_rb_gb"][0]["noise_energy"] = 100.0
            elif kind == "trace":
                variant["trace"]["terms"]["noise_clip"][0] = 100.0
            else:
                variant["trace"]["reference"][0] = 100.0
            with self.assertRaises(ValueError):
                verify_numerics(self.protocol, rows, self.old_rows)
        result = analyze(self.protocol, self.rows)
        for key in ("candidate_fitted", "classification_evaluable", "new_candidate_created", "release_ready"):
            self.assertFalse(result[key])


if __name__ == "__main__":
    unittest.main()
