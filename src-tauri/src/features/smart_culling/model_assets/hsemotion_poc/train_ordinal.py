#!/usr/bin/env python3
"""Fit and audit a calibration-only five-level ordinal expression head."""

from __future__ import annotations

import argparse
import hashlib
import json
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from scipy.stats import spearmanr

from ordinal_model import balanced_nll, fit, predict, select_hyperparameters, thresholds

MODEL_VERSION = "qraw-expression-quality-ordinal-candidate-0.1"
FEATURE_VERSION = "18-hse-raw-plus-log1p-38-non-eye-blendshapes"


@dataclass
class Dataset:
    rows: list[dict]
    features: np.ndarray
    labels: np.ndarray
    reliable: np.ndarray
    feature_names: list[str]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_dataset(path: Path) -> Dataset:
    rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    if not rows:
        raise RuntimeError(f"empty evidence report: {path}")
    feature_names: list[str] | None = None
    features = []
    labels = []
    reliable = []
    for row in rows:
        label = int(row["manualExpressionLabel"])
        if label not in range(1, 6):
            raise RuntimeError(f"manual label outside 1..5: {row['sampleId']}")
        evidence = row.get("expressionEvidence", {})
        quality = evidence.get("qualityModels", {})
        blendshapes = evidence.get("nonEyeBlendshapes", {})
        names = sorted(blendshapes)
        if row.get("status") != "ok" or quality.get("status") != "ok":
            raise RuntimeError(f"missing required model evidence: {row['sampleId']}")
        if len(names) != 38:
            raise RuntimeError(f"expected 38 non-eye blendshapes: {row['sampleId']}")
        current_names = [
            *(f"hse_mtl_{index}" for index in range(10)),
            *(f"hse_vgaf_{index}" for index in range(8)),
            *(f"log_blendshape_{name}" for name in names),
        ]
        if feature_names is None:
            feature_names = current_names
        elif feature_names != current_names:
            raise RuntimeError(f"feature order changed: {row['sampleId']}")
        hse = np.asarray(quality["mtl"] + quality["vgaf"], dtype=np.float64)
        non_eye = np.log1p(
            100.0 * np.asarray([blendshapes[name] for name in names], dtype=np.float64)
        )
        vector = np.concatenate((hse, non_eye))
        if vector.shape != (56,) or not np.all(np.isfinite(vector)):
            raise RuntimeError(f"invalid feature vector: {row['sampleId']}")
        features.append(vector)
        labels.append(label)
        reliable.append(bool(evidence.get("descriptorReliable")))
    sample_ids = [row["sampleId"] for row in rows]
    if len(sample_ids) != len(set(sample_ids)):
        raise RuntimeError(f"duplicate sample id in {path}")
    return Dataset(
        rows=rows,
        features=np.stack(features),
        labels=np.asarray(labels, dtype=np.int8),
        reliable=np.asarray(reliable, dtype=bool),
        feature_names=feature_names or [],
    )


def metrics(labels: np.ndarray, probabilities: np.ndarray) -> dict:
    predictions = np.argmax(probabilities, axis=1) + 1
    errors = np.abs(labels - predictions)
    confusion = [
        [int(np.sum((labels == actual) & (predictions == predicted))) for predicted in range(1, 6)]
        for actual in range(1, 6)
    ]
    expected = probabilities @ np.arange(1, 6)
    return {
        "count": len(labels),
        "exact": int(np.sum(errors == 0)),
        "exactRate": float(np.mean(errors == 0)),
        "withinOne": int(np.sum(errors <= 1)),
        "withinOneRate": float(np.mean(errors <= 1)),
        "crossLevelErrors": int(np.sum(errors >= 2)),
        "meanAbsoluteError": float(np.mean(errors)),
        "spearmanExpectedLabel": float(spearmanr(labels, expected).statistic),
        "classBalancedNegativeLogLikelihood": balanced_nll(labels, probabilities),
        "predictedClassCounts": {
            str(level): int(np.sum(predictions == level)) for level in range(1, 6)
        },
        "confusionMatrixActualRowsPredictedColumns": confusion,
    }


def prediction_rows(dataset: Dataset, probabilities: np.ndarray) -> list[dict]:
    output = []
    for row, reliable, values in zip(dataset.rows, dataset.reliable, probabilities):
        prediction = int(np.argmax(values) + 1) if reliable else None
        output.append(
            {
                "sampleId": row["sampleId"],
                "file": row["file"],
                "manualExpressionLabel": row["manualExpressionLabel"],
                "algorithmState": "five_level_candidate" if reliable else "unable_to_determine",
                "candidateLabel": prediction,
                "probabilities": values.tolist(),
                "absoluteError": abs(prediction - row["manualExpressionLabel"])
                if prediction is not None
                else None,
                "abstentionReason": None if reliable else "existing_reliability_gate",
            }
        )
    return output


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--training-evidence", type=Path, required=True)
    parser.add_argument("--evaluation-evidence", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    training = load_dataset(args.training_evidence)
    evaluation = load_dataset(args.evaluation_evidence)
    if training.feature_names != evaluation.feature_names:
        raise RuntimeError("training/evaluation feature order differs")
    if not np.all(training.reliable):
        raise RuntimeError("unreliable rows cannot train this candidate")
    components, l2, candidates, cross_validated = select_hyperparameters(
        training.features,
        training.labels,
        [row["sampleId"] for row in training.rows],
    )
    model = fit(training.features, training.labels, components, l2)
    evaluation_probabilities = predict(model, evaluation.features)
    reliable_probabilities = evaluation_probabilities[evaluation.reliable]
    result = {
        "schemaVersion": "qraw-expression-ordinal-candidate-report-1.0",
        "modelVersion": MODEL_VERSION,
        "status": "calibration_candidate_not_for_production",
        "protocol": {
            "target": "manual expression labels 1..5 as an ordered outcome",
            "model": "class-balanced proportional-odds logistic regression",
            "features": FEATURE_VERSION,
            "dimensionReduction": "training-only standardized PCA",
            "selection": "lowest five-fold class-balanced negative log-likelihood on batch 001 only",
            "foldingLimitation": "subject_id and capture_group_id are unresolved; folds are stratified by label and sample hash only",
            "abstention": "existing descriptor reliability gate; no candidate confidence threshold is frozen",
        },
        "inputs": {
            "trainingEvidenceSha256": sha256(args.training_evidence),
            "evaluationEvidenceSha256": sha256(args.evaluation_evidence),
            "trainingCount": len(training.rows),
            "evaluationCount": len(evaluation.rows),
            "evaluationReliableCount": int(np.sum(evaluation.reliable)),
            "evaluationAbstentionCount": int(np.sum(~evaluation.reliable)),
        },
        "selection": {
            "selectedPcaComponents": components,
            "selectedL2": l2,
            "candidates": candidates,
            "batch001FiveFoldDiagnostic": metrics(training.labels, cross_validated),
        },
        "batch002CrossBatchDiagnostic": metrics(
            evaluation.labels[evaluation.reliable], reliable_probabilities
        ),
        "limitations": [
            "Batch 002 labels were already revealed and inspected, so this is not a strict blind result.",
            "Neither batch has confirmed subject_id/capture_group_id isolation.",
            "Known AI-generated images and unknown-source images cannot prove real-camera generalization.",
            "The candidate is not connected to production scoring and has no release authority.",
        ],
        "model": {
            "featureNames": training.feature_names,
            "means": model.means.tolist(),
            "stds": model.stds.tolist(),
            "pcaComponents": model.components.tolist(),
            "weights": model.parameters[:components].tolist(),
            "thresholds": thresholds(model).tolist(),
        },
        "predictions": prediction_rows(evaluation, evaluation_probabilities),
    }
    if not args.output.is_absolute() or not args.output.parent.is_dir():
        raise RuntimeError("output must be a new absolute file in an existing directory")
    with args.output.open("x", encoding="utf-8") as file:
        json.dump(result, file, ensure_ascii=False, indent=2)
        file.write("\n")
    print(json.dumps(result["batch002CrossBatchDiagnostic"], ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
