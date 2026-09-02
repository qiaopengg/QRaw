"""Small proportional-odds model used by the expression calibration CLI."""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
from scipy.optimize import minimize
from scipy.special import expit


PCA_COMPONENT_CANDIDATES = (5, 8, 10, 12, 15, 20, 25, 30)
L2_CANDIDATES = (0.003, 0.01, 0.03, 0.1, 0.3, 1.0)
FOLD_COUNT = 5
MINIMUM_THRESHOLD_GAP = 0.05


@dataclass
class Model:
    means: np.ndarray
    stds: np.ndarray
    components: np.ndarray
    parameters: np.ndarray
    l2: float


def thresholds(model: Model) -> np.ndarray:
    return _thresholds(model.parameters, model.components.shape[0])


def fit(features: np.ndarray, labels: np.ndarray, component_count: int, l2: float) -> Model:
    means = features.mean(axis=0)
    stds = features.std(axis=0)
    stds[stds < 1e-6] = 1.0
    standardized = (features - means) / stds
    components = _canonical_components(standardized, component_count)
    projected = np.einsum("ij,kj->ik", standardized, components)
    class_counts = np.bincount(labels, minlength=6)[1:]
    if np.any(class_counts == 0):
        raise RuntimeError("ordinal fit requires all five manual labels")
    sample_weights = len(labels) / (5.0 * class_counts[labels - 1])
    sample_weights /= sample_weights.sum()
    cumulative = np.asarray([(labels <= level).mean() for level in range(1, 5)])
    initial_thresholds = np.log(cumulative / (1.0 - cumulative))
    initial_gaps = np.maximum(np.diff(initial_thresholds) - MINIMUM_THRESHOLD_GAP, 1e-3)
    initial = np.concatenate(
        (
            np.zeros(component_count),
            [initial_thresholds[0]],
            np.log(np.expm1(initial_gaps)),
        )
    )

    def objective(parameters: np.ndarray) -> tuple[float, np.ndarray]:
        weights = parameters[:component_count]
        cumulative_probabilities = expit(
            _thresholds(parameters, component_count)[None, :]
            - np.einsum("ij,j->i", projected, weights)[:, None]
        )
        derivatives = cumulative_probabilities * (1.0 - cumulative_probabilities)
        boundaries = np.column_stack(
            (np.zeros(len(labels)), cumulative_probabilities, np.ones(len(labels)))
        )
        probabilities = np.maximum(
            boundaries[np.arange(len(labels)), labels]
            - boundaries[np.arange(len(labels)), labels - 1],
            1e-12,
        )
        loss = np.sum(sample_weights * -np.log(probabilities))
        loss += 0.5 * l2 * np.dot(weights, weights)

        upper = np.zeros(len(labels))
        lower = np.zeros(len(labels))
        indices = np.flatnonzero(labels <= 4)
        upper[indices] = derivatives[indices, labels[indices] - 1]
        indices = np.flatnonzero(labels >= 2)
        lower[indices] = derivatives[indices, labels[indices] - 2]
        weight_gradient = np.einsum(
            "ij,i->j",
            projected,
            sample_weights * (upper - lower) / probabilities,
        )
        weight_gradient += l2 * weights

        threshold_gradient = np.zeros(4)
        for level in range(1, 5):
            indices = np.flatnonzero(labels == level)
            threshold_gradient[level - 1] -= np.sum(
                sample_weights[indices]
                * derivatives[indices, level - 1]
                / probabilities[indices]
            )
            indices = np.flatnonzero(labels == level + 1)
            threshold_gradient[level - 1] += np.sum(
                sample_weights[indices]
                * derivatives[indices, level - 1]
                / probabilities[indices]
            )
        raw_gap_gradient = np.asarray(
            [
                expit(parameters[component_count + 1 + index])
                * threshold_gradient[index + 1 :].sum()
                for index in range(3)
            ]
        )
        gradient = np.concatenate(
            (weight_gradient, [threshold_gradient.sum()], raw_gap_gradient)
        )
        if not np.isfinite(loss) or not np.all(np.isfinite(gradient)):
            raise RuntimeError("ordinal optimizer produced non-finite values")
        return float(loss), gradient

    result = minimize(
        lambda parameters: objective(parameters),
        initial,
        jac=True,
        method="L-BFGS-B",
        bounds=[(-5.0, 5.0)] * component_count
        + [(-10.0, 10.0)]
        + [(-10.0, 5.0)] * 3,
        options={"maxiter": 3000, "ftol": 1e-12, "gtol": 1e-8},
    )
    if not result.success:
        raise RuntimeError(f"ordinal optimizer failed: {result.message}")
    return Model(means, stds, components, result.x, l2)


def predict(model: Model, features: np.ndarray) -> np.ndarray:
    projected = np.einsum(
        "ij,kj->ik", (features - model.means) / model.stds, model.components
    )
    score = np.einsum(
        "ij,j->i", projected, model.parameters[: model.components.shape[0]]
    )
    cumulative = expit(thresholds(model)[None, :] - score[:, None])
    probabilities = np.column_stack(
        (cumulative[:, 0], np.diff(cumulative, axis=1), 1.0 - cumulative[:, -1])
    )
    if np.any(probabilities < -1e-9) or not np.all(np.isfinite(probabilities)):
        raise RuntimeError("invalid ordinal probabilities")
    return np.maximum(probabilities, 0.0)


def balanced_nll(labels: np.ndarray, probabilities: np.ndarray) -> float:
    losses = -np.log(np.maximum(probabilities[np.arange(len(labels)), labels - 1], 1e-12))
    return float(np.mean([losses[labels == level].mean() for level in range(1, 6)]))


def select_hyperparameters(
    features: np.ndarray, labels: np.ndarray, sample_ids: list[str]
) -> tuple[int, float, list[dict], np.ndarray]:
    folds = _stratified_folds(labels, sample_ids)
    candidates = []
    predictions_by_candidate: dict[tuple[int, float], np.ndarray] = {}
    for component_count in PCA_COMPONENT_CANDIDATES:
        for l2 in L2_CANDIDATES:
            probabilities = np.zeros((len(labels), 5), dtype=np.float64)
            for fold in range(FOLD_COUNT):
                training = folds != fold
                held_out = ~training
                model = fit(features[training], labels[training], component_count, l2)
                probabilities[held_out] = predict(model, features[held_out])
            candidates.append(
                {
                    "pcaComponents": component_count,
                    "l2": l2,
                    "classBalancedNegativeLogLikelihood": balanced_nll(
                        labels, probabilities
                    ),
                }
            )
            predictions_by_candidate[(component_count, l2)] = probabilities
    selected = min(
        candidates,
        key=lambda row: (
            row["classBalancedNegativeLogLikelihood"],
            row["pcaComponents"],
            -row["l2"],
        ),
    )
    key = (selected["pcaComponents"], selected["l2"])
    return key[0], key[1], candidates, predictions_by_candidate[key]


def _thresholds(parameters: np.ndarray, dimension: int) -> np.ndarray:
    base = parameters[dimension]
    gaps = np.logaddexp(0.0, parameters[dimension + 1 :]) + MINIMUM_THRESHOLD_GAP
    return np.concatenate(([base], base + np.cumsum(gaps)))


def _canonical_components(standardized: np.ndarray, count: int) -> np.ndarray:
    _, _, right = np.linalg.svd(standardized, full_matrices=False)
    components = right[:count].copy()
    for component in components:
        pivot = int(np.argmax(np.abs(component)))
        if component[pivot] < 0.0:
            component *= -1.0
    return components


def _stratified_folds(labels: np.ndarray, sample_ids: list[str]) -> np.ndarray:
    folds = np.empty(len(labels), dtype=np.int8)
    sample_ids_array = np.asarray(sample_ids)
    for label in range(1, 6):
        indices = np.flatnonzero(labels == label)
        indices = indices[np.argsort(sample_ids_array[indices])]
        for offset, index in enumerate(indices):
            folds[index] = offset % FOLD_COUNT
    return folds
