"""Version 0.3 offline hypothesis; reuse the existing gap fitter, never runtime scoring."""

import json
from pathlib import Path

from candidate import finite, fit as fit_gap, predict as predict_gap

METHOD = json.loads(Path(__file__).with_name("linear_protocol.json").read_text())
VERSION = METHOD["candidate_version"]
FEATURES = METHOD["features"]
INPUT_SOURCE = METHOD["input_source"]


def detail_row(row):
    measured = row.get("linear_detail") or {}
    scales = measured.get("scales") or []
    if (measured.get("version") != METHOD["measurement_version"]
            or row.get("unavailable_reason") is not None
            or len(scales) != 2 or [s.get("smoothingOrder") for s in scales] != [2, 4]
            or not finite(measured.get("noiseSigma")) or measured["noiseSigma"] < 0
            or any(not finite(s.get("correctedEnergy")) or s["correctedEnergy"] <= 0 for s in scales)):
        return dict(row, detail={})
    return dict(row, detail=dict(fineEnergy=scales[0]["correctedEnergy"],
                                coarseToFineRatio=measured.get("coarseToFineRatio")))


def fit(rows):
    if any(row.get("input_source") != INPUT_SOURCE for row in rows):
        raise ValueError("候选 0.3 只能使用来源明确的原图拟合")
    candidate = fit_gap([detail_row(row) for row in rows], features=FEATURES)
    candidate.update(version=VERSION, input_source=INPUT_SOURCE, fit_status="fitted",
        measurement_version=METHOD["measurement_version"], method=METHOD["method"])
    return candidate


def predict(row, candidate):
    if (candidate.get("version") != VERSION or candidate.get("fit_status") != "fitted"
            or candidate.get("measurement_version") != METHOD["measurement_version"]
            or candidate.get("input_source") != INPUT_SOURCE):
        return "uncertain"
    return predict_gap(detail_row(row), candidate, features=FEATURES)
