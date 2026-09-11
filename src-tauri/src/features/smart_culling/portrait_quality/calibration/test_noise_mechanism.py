import json
import tempfile
import unittest
from pathlib import Path

from legacy import content_hash
from noise_mechanism import checked_read, matrix, prepare


class NoiseMechanismTests(unittest.TestCase):
    def test_fixed_matrix_keeps_all_sizes_seeds_and_float_source_boundaries(self):
        cases = matrix()
        self.assertEqual(len(cases), 495)
        self.assertEqual(len({case["id"] for case in cases}), 495)
        self.assertEqual({case["seed"] for case in cases}, {0x51455741, 1, 20260910})
        self.assertTrue(all(case["side"] == 512 for case in cases if "resize" in case["processing"]))
        self.assertEqual(sum(case["processing"] == "unclipped_float" for case in cases), 135)
        self.assertFalse(any("float" in case["processing"] and "sharpen" in case["processing"] for case in cases))

    def test_partial_matrix_changed_design_and_duplicate_preparation_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / "PROTOCOL.md").write_text("fixed design")
            prepare(directory)
            protocol = json.loads((directory / "PROTOCOL.json").read_text())
            observed = dict(schema="qraw-noise-mechanism-observations-1",
                protocol_sha256=content_hash(directory / "PROTOCOL.json"), code_hashes=protocol["code_hashes"],
                rows=[dict(case=case) for case in matrix()])
            (directory / "observations.json").write_text(json.dumps(observed))
            self.assertEqual(len(checked_read(directory)[1]), 495)
            with self.assertRaises(FileExistsError):
                prepare(directory)
            observed["rows"].pop()
            (directory / "observations.json").write_text(json.dumps(observed))
            with self.assertRaises(ValueError):
                checked_read(directory)
            observed["rows"] = [dict(case=case) for case in matrix()]
            (directory / "observations.json").write_text(json.dumps(observed))
            (directory / "PROTOCOL.md").write_text("changed design")
            with self.assertRaises(ValueError):
                checked_read(directory)


if __name__ == "__main__":
    unittest.main()
