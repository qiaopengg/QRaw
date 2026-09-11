"""Offline 0.2 hypothesis: fixed median preprocessing, unchanged extrema-gap fitter."""

from candidate import finite, fit as fit_gap, predict as predict_gap

VERSION = "qraw-clarity-median-gap-candidate-0.2"
MEASUREMENT_VERSION = "qraw-face-robust-detail-1"
INPUT_SOURCE = "decodedRgb8LosslessGeometry"


def detail_row(row):
    robust = row.get("robust_detail") or {}
    if (robust.get("version") != MEASUREMENT_VERSION or robust.get("medianRadius") != 2
            or not finite(robust.get("noiseSigma")) or robust["noiseSigma"] < 0
            or row.get("unavailable_reason") is not None):
        return dict(row, detail={})
    return dict(row, detail=robust.get("denoisedDetail") or {})


def fit(rows):
    if any(row.get("input_source") != INPUT_SOURCE for row in rows):
        raise ValueError("候选 0.2 只能使用明确的原图清晰度输入拟合")
    candidate = fit_gap([detail_row(row) for row in rows])
    candidate.update(version=VERSION, input_source=INPUT_SOURCE,
        measurement_version=MEASUREMENT_VERSION, median_radius=2,
        noise_sigma_used_for_threshold=False,
        method="fixed 5x5 luminance median before existing local detail; same extrema-gap fitter as 0.1")
    return candidate


def predict(row, candidate):
    if (candidate.get("version") != VERSION or candidate.get("measurement_version") != MEASUREMENT_VERSION
            or candidate.get("input_source") != INPUT_SOURCE or candidate.get("median_radius") != 2):
        return "uncertain"
    return predict_gap(detail_row(row), candidate)
