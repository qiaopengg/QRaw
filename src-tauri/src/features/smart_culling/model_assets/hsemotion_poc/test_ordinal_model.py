import unittest

import numpy as np

from ordinal_model import balanced_nll, fit, predict, thresholds
from train_ordinal import parse_args


class OrdinalModelTest(unittest.TestCase):
    def test_candidate_metadata_arguments_are_explicit(self) -> None:
        args = parse_args(
            [
                "--training-evidence",
                "/tmp/training.jsonl",
                "--evaluation-evidence",
                "/tmp/evaluation.jsonl",
                "--output",
                "/tmp/report.json",
                "--model-version",
                "qraw-expression-quality-ordinal-candidate-0.2",
                "--training-dataset",
                "batch001+batch002-reliable",
                "--evaluation-dataset",
                "batch003-synthetic",
            ]
        )

        self.assertEqual(
            args.model_version, "qraw-expression-quality-ordinal-candidate-0.2"
        )
        self.assertEqual(args.training_dataset, "batch001+batch002-reliable")
        self.assertEqual(args.evaluation_dataset, "batch003-synthetic")

    def test_fit_produces_ordered_probabilities(self) -> None:
        labels = np.repeat(np.arange(1, 6, dtype=np.int8), 12)
        centered = labels.astype(np.float64) - 3.0
        offsets = np.tile(np.linspace(-0.12, 0.12, 12), 5)
        features = np.column_stack(
            (
                centered + offsets,
                0.5 * centered - offsets,
                centered**2 + 0.1 * offsets,
            )
        )
        model = fit(features, labels, component_count=2, l2=0.03)
        probabilities = predict(model, features)

        self.assertTrue(np.all(np.diff(thresholds(model)) > 0.0))
        self.assertTrue(np.all(probabilities >= 0.0))
        np.testing.assert_allclose(probabilities.sum(axis=1), 1.0, atol=1e-12)
        self.assertLess(np.mean(np.abs(np.argmax(probabilities, axis=1) + 1 - labels)), 0.2)

    def test_balanced_nll_penalizes_wrong_order(self) -> None:
        labels = np.arange(1, 6, dtype=np.int8)
        correct = np.eye(5) * 0.8 + 0.04
        reversed_probabilities = correct[::-1]

        self.assertLess(
            balanced_nll(labels, correct),
            balanced_nll(labels, reversed_probabilities),
        )


if __name__ == "__main__":
    unittest.main()
