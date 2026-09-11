import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import numpy as np
import torch

from export_geometry import fixed_geometry
from network_baseline import check_outputs, run
from official import SOURCE_HASHES, checked_tree, configuration
from verify_network import baseline_cases, compare_outputs


class NetworkContractTest(unittest.TestCase):
    def test_source_drift_is_rejected_before_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "config.py").write_text("raise RuntimeError('must not execute')")
            with self.assertRaisesRegex(ValueError, "source hash mismatch"):
                checked_tree(root, "config.py")

    def test_configuration_does_not_execute_training_filesystem_checks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = b"class Config:\n    assert False\n    score_level = 5\n    attribute_types = ['a', 'b']\n"
            (root / "config.py").write_bytes(data)
            with patch.dict(SOURCE_HASHES, {"config.py": hashlib.sha256(data).hexdigest()}):
                cfg = configuration(root)
            self.assertEqual(cfg.score_level, 5)
            self.assertEqual(cfg.num_attributes, 2)

    def test_invalid_and_incomplete_outputs_are_not_accepted(self):
        outputs = [torch.zeros(1, 8), torch.zeros(1, 6), torch.full((1, 5), 0.2)]
        check_outputs(outputs)
        outputs[2][0, 0] = float("nan")
        with self.assertRaises(ValueError):
            check_outputs(outputs)
        with self.assertRaises(ValueError):
            compare_outputs({"a": [[1]]}, {}, atol=1e-4, rtol=1e-5)
        with self.assertRaises(AssertionError):
            compare_outputs({"a": [[1]]}, {"a": np.array([[1.01]])}, atol=1e-4, rtol=1e-5)

    def test_incomplete_baseline_and_existing_output_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "REPORT.json").write_text(json.dumps({"passed": False}))
            with self.assertRaises(ValueError):
                list(baseline_cases(root))
            before = (root / "REPORT.json").read_bytes()
            with self.assertRaises(FileExistsError):
                run(root, root / "PROTOCOL.json", root)
            self.assertEqual((root / "REPORT.json").read_bytes(), before)

    def test_static_triangle_indices_match_original_and_restore(self):
        original = torch.triu_indices
        references = {(name, size): getattr(torch, name)(size, size, offset=offset)
                      for name, offset in [("triu_indices", 1), ("tril_indices", -1)] for size in (7, 42)}
        with fixed_geometry():
            for (name, size), expected in references.items():
                actual = getattr(torch, name)(size, size, offset=1 if name == "triu_indices" else -1)
                torch.testing.assert_close(actual, expected, atol=0, rtol=0)
            with self.assertRaises(ValueError):
                torch.triu_indices(8, 8, offset=1)
        self.assertIs(torch.triu_indices, original)


if __name__ == "__main__":
    unittest.main()
