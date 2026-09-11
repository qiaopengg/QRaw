import hashlib
import tempfile
import unittest
from pathlib import Path

from release_inventory import RUNTIME_CONSTANTS, inspect, string_constant


class ReleaseInventoryTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.assets = self.root / "model_assets"
        self.assets.mkdir()
        self.resources = self.root / "resources"
        self.resources.mkdir()
        for source_name, constants in RUNTIME_CONSTANTS.items():
            declarations = []
            for name, digest in constants:
                filename = f"{name}.onnx"
                data = name.encode()
                (self.assets / filename).write_bytes(data)
                declarations.extend([f'const {name}: &str = "{filename}";',
                                     f'const {digest}: &str =\n "{hashlib.sha256(data).hexdigest()}";'])
            (self.root / source_name).write_text("\n".join(declarations))
        (self.resources / "base.onnx").write_bytes(b"independent bundled model")

    def test_isolation_success_does_not_claim_release_acceptance(self):
        report = inspect(self.root, self.resources)
        self.assertTrue(report["integrity_passed"])
        self.assertTrue(report["packaging_isolated"])
        self.assertEqual(len(report["models"]), 4)
        self.assertFalse(report["release_readiness_assessed"])
        self.assertIn("independent real-camera acceptance", report["remaining"])

    def test_modified_and_missing_runtime_models_fail_integrity(self):
        paths = sorted(self.assets.iterdir())
        paths[0].write_bytes(b"changed")
        paths[1].unlink()
        report = inspect(self.root, self.resources)
        self.assertFalse(report["integrity_passed"])
        self.assertEqual(sum(not row["matches"] for row in report["models"]), 2)

    def test_renamed_calibration_bytes_are_detected_in_nested_resources(self):
        path = next(self.assets.iterdir())
        nested = self.resources / "nested"
        nested.mkdir()
        (nested / "renamed.ONNX").write_bytes(path.read_bytes())
        report = inspect(self.root, self.resources)
        self.assertFalse(report["packaging_isolated"])

    def test_calibration_filename_is_rejected_even_with_changed_bytes(self):
        path = next(self.assets.iterdir())
        (self.resources / path.name).write_bytes(b"different weight")
        self.assertFalse(inspect(self.root, self.resources)["packaging_isolated"])

    def test_missing_or_empty_resources_do_not_count_as_isolation(self):
        (self.resources / "base.onnx").unlink()
        self.assertFalse(inspect(self.root, self.resources)["packaging_isolated"])
        with self.assertRaises(ValueError):
            inspect(self.root, self.root / "missing")

    def test_source_drift_fails_loudly_instead_of_skipping_models(self):
        (self.root / "face_motion_poc.rs").write_text("// contract removed")
        with self.assertRaises(ValueError):
            inspect(self.root, self.resources)
        with self.assertRaises(ValueError):
            string_constant('const A: &str = "one"; const A: &str = "two";', "A")


if __name__ == "__main__":
    unittest.main()
