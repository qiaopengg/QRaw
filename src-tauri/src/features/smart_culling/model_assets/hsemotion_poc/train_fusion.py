#!/usr/bin/env python3
"""Fit and audit the isolated HSEmotion + non-eye Blendshape fusion head."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path

import numpy as np
from scipy.optimize import minimize


L2_STRENGTH = 0.1
CALIBRATION_WEIGHT = 3.0
POSITIVE_GUARD_WEIGHT = 100.0
POSITIVE_GUARD_MARGIN = 1e-6


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_rows(path: Path, kind: str) -> list[dict]:
    rows = []
    with path.open(encoding="utf-8") as file:
        for line in file:
            report = json.loads(line)
            motion = report["rawEvidence"]["faceMotion"]
            model = motion.get("expressionQualityModel", {})
            if (
                motion.get("status") != "ok"
                or model.get("status") != "ok"
                or motion.get("expression", {}).get("score") is None
            ):
                continue
            blendshapes = motion["blendshapes"]
            names = sorted(name for name in blendshapes if not name.startswith("eye"))
            if len(names) != 38:
                raise RuntimeError(f"expected 38 non-eye blendshapes in {path}")
            hse = np.asarray(model["mtl"] + model["vgaf"], dtype=np.float64)
            if hse.shape != (18,):
                raise RuntimeError(f"expected 18 HSE outputs in {path}")
            non_eye = np.log1p(
                100.0 * np.asarray([blendshapes[name] for name in names], dtype=np.float64)
            )
            if not np.all(np.isfinite(hse)) or not np.all(np.isfinite(non_eye)):
                raise RuntimeError(f"non-finite fusion feature in {path}")
            rating = int(report["manual"]["rating"])
            rows.append(
                {
                    "file": report["file"],
                    "kind": kind,
                    "rating": rating,
                    "label": int(rating >= 3),
                    "features": np.concatenate((hse, non_eye)),
                    "feature_names": [
                        *(f"hse_mtl_{index}" for index in range(10)),
                        *(f"hse_vgaf_{index}" for index in range(8)),
                        *(f"log_blendshape_{name}" for name in names),
                    ],
                }
            )
    return rows


def group_name(row: dict) -> str:
    if row["kind"] == "expression":
        return row["file"].split("_", 1)[0]
    if row["kind"] == "incremental":
        return "INCREMENTAL_CALIBRATION"
    match = re.search(r"(\d+)", row["file"])
    if match is None:
        raise RuntimeError(f"calibration filename has no sequence number: {row['file']}")
    number = int(match.group(1))
    return f"CAL_{number:04d}" if number <= 20 else f"CAL_PAIR_{(number - 21) // 2 + 1}"


def fit(rows: list[dict], guards: list[dict]) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    features = np.stack([row["features"] for row in rows])
    labels = np.asarray([row["label"] for row in rows], dtype=np.float64)
    means = features.mean(axis=0)
    stds = features.std(axis=0)
    stds[stds < 1e-6] = 1.0
    standardized = (features - means) / stds
    guard_features = (np.stack([row["features"] for row in guards]) - means) / stds

    count = len(labels)
    negative_count = np.sum(labels == 0)
    positive_count = np.sum(labels == 1)
    sample_weights = np.where(
        labels == 0,
        count / (2.0 * negative_count),
        count / (2.0 * positive_count),
    )
    sample_weights *= np.asarray(
        [
            CALIBRATION_WEIGHT
            if row["kind"] in ("calibration", "incremental")
            else 1.0
            for row in rows
        ]
    )
    sample_weights /= sample_weights.sum()

    def objective(parameters: np.ndarray) -> tuple[float, np.ndarray]:
        if not np.all(np.isfinite(parameters)) or np.any(np.abs(parameters[:-1]) > 8.000001):
            raise RuntimeError("fusion optimizer left its finite parameter bounds")
        weights = parameters[:-1]
        bias = parameters[-1]
        logits = np.einsum("ij,j->i", standardized, weights) + bias
        probabilities = 1.0 / (1.0 + np.exp(-np.clip(logits, -40.0, 40.0)))
        loss = np.sum(
            sample_weights * (np.logaddexp(0.0, logits) - labels * logits)
        ) + 0.5 * L2_STRENGTH * np.dot(weights, weights)
        weight_gradient = np.einsum(
            "ij,i->j", standardized, sample_weights * (probabilities - labels)
        )
        weight_gradient += L2_STRENGTH * weights
        bias_gradient = np.sum(sample_weights * (probabilities - labels))

        guard_logits = np.einsum("ij,j->i", guard_features, weights) + bias
        hinge = np.maximum(0.0, -guard_logits)
        loss += POSITIVE_GUARD_WEIGHT * np.mean(hinge**2)
        active = hinge > 0.0
        if np.any(active):
            coefficients = -2.0 * POSITIVE_GUARD_WEIGHT * hinge[active] / len(hinge)
            weight_gradient += np.einsum(
                "ij,i->j", guard_features[active], coefficients
            )
            bias_gradient += coefficients.sum()
        return loss, np.concatenate((weight_gradient, [bias_gradient]))

    result = minimize(
        lambda parameters: objective(parameters),
        np.zeros(features.shape[1] + 1),
        jac=True,
        method="L-BFGS-B",
        bounds=[(-8.0, 8.0)] * features.shape[1] + [(-20.0, 20.0)],
        options={"maxiter": 3000, "ftol": 1e-13, "gtol": 1e-9},
    )
    if not result.success:
        raise RuntimeError(f"fusion optimizer failed: {result.message}")
    parameters = result.x
    minimum_guard_logit = np.min(
        np.einsum("ij,j->i", guard_features, parameters[:-1]) + parameters[-1]
    )
    parameters[-1] += max(0.0, -minimum_guard_logit + POSITIVE_GUARD_MARGIN)
    return means, stds, parameters


def predict(rows: list[dict], model: tuple[np.ndarray, np.ndarray, np.ndarray]) -> np.ndarray:
    means, stds, parameters = model
    standardized = (np.stack([row["features"] for row in rows]) - means) / stds
    logits = np.einsum("ij,j->i", standardized, parameters[:-1]) + parameters[-1]
    return 1.0 / (1.0 + np.exp(-np.clip(logits, -40.0, 40.0)))


def metrics(rows: list[dict], probabilities: np.ndarray) -> dict:
    labels = np.asarray([row["label"] for row in rows], dtype=np.int8)
    predictions = probabilities >= 0.5
    positives = np.flatnonzero(labels == 1)
    negatives = np.flatnonzero(labels == 0)
    auc = np.mean(
        [
            (probabilities[left] > probabilities[right])
            + 0.5 * (probabilities[left] == probabilities[right])
            for left in positives
            for right in negatives
        ]
    )
    return {
        "count": len(rows),
        "accuracy": float(np.mean(predictions == labels)),
        "auc": float(auc),
        "truePositive": int(np.sum(predictions & (labels == 1))),
        "trueNegative": int(np.sum(~predictions & (labels == 0))),
        "falsePositive": int(np.sum(predictions & (labels == 0))),
        "falseNegative": int(np.sum(~predictions & (labels == 1))),
    }


def grouped_predictions(rows: list[dict], other: list[dict], guards: list[dict]) -> np.ndarray:
    probabilities = np.zeros(len(rows), dtype=np.float64)
    for group in sorted({group_name(row) for row in rows}):
        held_out = [index for index, row in enumerate(rows) if group_name(row) == group]
        training = [row for row in rows if group_name(row) != group] + other
        probabilities[held_out] = predict([rows[index] for index in held_out], fit(training, guards))
    return probabilities


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--expression-report", type=Path, required=True)
    parser.add_argument("--calibration-report", type=Path, required=True)
    parser.add_argument("--incremental-calibration-report", type=Path, required=True)
    parser.add_argument("--positive-guard-report", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    expression = load_rows(args.expression_report, "expression")
    calibration = load_rows(args.calibration_report, "calibration")
    incremental = load_rows(args.incremental_calibration_report, "incremental")
    guards = [
        row
        for row in load_rows(args.positive_guard_report, "guard")
        if row["rating"] >= 3
    ]
    if (len(expression), len(calibration), len(incremental), len(guards)) != (
        105,
        28,
        10,
        125,
    ):
        raise RuntimeError(
            "calibration population changed: "
            f"{len(expression)} expression, {len(calibration)} calibration, "
            f"{len(incremental)} incremental, {len(guards)} guard"
        )
    feature_names = expression[0]["feature_names"]
    if any(
        row["feature_names"] != feature_names
        for row in expression + calibration + incremental + guards
    ):
        raise RuntimeError("fusion feature order changed")

    expression_probabilities = grouped_predictions(
        expression, calibration + incremental, guards
    )
    calibration_probabilities = grouped_predictions(
        calibration, expression + incremental, guards
    )
    incremental_selection_probabilities = grouped_predictions(
        incremental, expression + calibration, guards
    )
    final_model = fit(expression + calibration + incremental, guards)
    incremental_fit_probabilities = predict(incremental, final_model)
    guard_probabilities = predict(guards, final_model)
    third_batch = [
        (row, probability)
        for row, probability in zip(calibration, calibration_probabilities)
        if int(re.search(r"(\d+)", row["file"]).group(1)) >= 21
    ]
    output = {
        "version": "qraw-expression-quality-hsemotion-fusion-calibration-0.5",
        "protocol": {
            "label": "rating >= 3",
            "features": "18 raw HSE outputs + log1p(100 * 38 non-eye blendshapes)",
            "l2": L2_STRENGTH,
            "calibrationWeight": CALIBRATION_WEIGHT,
            "positiveGuardWeight": POSITIVE_GUARD_WEIGHT,
            "grouping": "leave-one-capture-group-out",
            "incrementalCalibrationRole": "model selection and calibration; not blind test",
        },
        "inputs": {
            "expressionReport": sha256(args.expression_report),
            "calibrationReport": sha256(args.calibration_report),
            "incrementalCalibrationReport": sha256(
                args.incremental_calibration_report
            ),
            "positiveGuardReport": sha256(args.positive_guard_report),
        },
        "metrics": {
            "expressionGroupedHoldout": metrics(expression, expression_probabilities),
            "calibrationGroupedHoldout": metrics(calibration, calibration_probabilities),
            "thirdBatchGroupedHoldout": metrics(
                [row for row, _ in third_batch],
                np.asarray([probability for _, probability in third_batch]),
            ),
            "incrementalSelectionReplay": metrics(
                incremental, incremental_selection_probabilities
            ),
            "incrementalCalibrationFit": metrics(
                incremental, incremental_fit_probabilities
            ),
            "positiveGuard": {
                "passed": int(np.sum(guard_probabilities >= 0.5)),
                "count": len(guards),
                "minimumScore": float(np.min(guard_probabilities)),
            },
        },
        "featureNames": feature_names,
        "constants": {
            "means": final_model[0].tolist(),
            "stds": final_model[1].tolist(),
            "weights": final_model[2][:-1].tolist(),
            "bias": float(final_model[2][-1]),
        },
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(output["metrics"], ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
