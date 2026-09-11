"""Offline 0.4 spatial-width candidate; one existing gap fit, no runtime classifier."""

import json
from pathlib import Path

from candidate import finite, fit as fit_gap, predict as predict_gap

METHOD = json.loads(Path(__file__).with_name("edge_protocol.json").read_text())
VERSION = METHOD["candidate_version"]
FEATURES = METHOD["features"]
INPUT_SOURCE = METHOD["input_source"]


def detail_row(row):
    measured = row.get("edge_spread") or {}
    if (measured.get("version") != METHOD["measurement_version"]
            or row.get("unavailable_reason") is not None
            or type(measured.get("acceptedProfiles")) is not int or measured["acceptedProfiles"] < 6
            or not finite(measured.get("widthP25")) or measured["widthP25"] <= 0):
        return dict(row, detail={})
    return dict(row, detail={"widthP25": measured["widthP25"]})


def fit(rows):
    if any(row.get("input_source") != INPUT_SOURCE for row in rows):
        raise ValueError("候选 0.4 只能使用来源明确的原图拟合")
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
