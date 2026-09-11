"""Offline gap-based clarity candidate. Not imported by any application rating path."""

import math

# Correlated detail measurements, not three independent votes. FIQA is auxiliary only.
FEATURES = {"nativeLaplacian": 1, "scaledLaplacian": 1, "blurRetentionRatio": -1}


def finite(value):
    return type(value) in (int, float) and math.isfinite(value)


def fit(rows, features=FEATURES):
    known = [row for row in rows if row["label"] in (1, 2)]
    thresholds = {}
    for name, direction in features.items():
        if any(not finite(row["detail"].get(name)) for row in known):
            raise ValueError(f"拟合证据缺失：{name}；不删除困难样本")
        clear = [direction * row["detail"][name] for row in known if row["label"] == 1]
        blurry = [direction * row["detail"][name] for row in known if row["label"] == 2]
        if not clear or not blurry:
            raise ValueError("缺少清晰或模糊真值，不能拟合")
        lower, upper = max(blurry), min(clear)
        if lower >= upper:
            raise ValueError(f"{name} 没有分离间隔；拒绝硬凑当前候选")
        gap = upper - lower
        thresholds[name] = dict(direction=direction, blurry=direction * (lower + 0.25 * gap),
                                clear=direction * (lower + 0.75 * gap))
    return dict(version="qraw-clarity-detail-gap-candidate-0.1", input_source="decodedRgb8",
                method="per-feature oriented class-extrema gap; middle half abstains; all features must agree",
                thresholds=thresholds, runtime_enabled=False, release_ready=False,
                independent_validation=False, calibration_count=len(known))


def predict(row, candidate, features=FEATURES):
    bbox = row.get("bbox")
    if (row.get("input_source") != candidate["input_source"] or not bbox or len(bbox) != 4
            or not all(finite(v) for v in bbox) or bbox[2] <= 0 or bbox[3] <= 0
            or set(candidate["thresholds"]) != set(features)):
        return "uncertain"
    clear, blurry = True, True
    for name, bounds in candidate["thresholds"].items():
        value = row.get("detail", {}).get(name)
        if (not finite(value) or value < 0 or bounds["direction"] != features[name]
                or not finite(bounds["clear"]) or not finite(bounds["blurry"])):
            return "uncertain"
        direction = bounds["direction"]
        if direction * bounds["blurry"] >= direction * bounds["clear"]:
            return "uncertain"
        clear &= direction * value >= direction * bounds["clear"]
        blurry &= direction * value <= direction * bounds["blurry"]
    return "clear" if clear else "unclear" if blurry else "uncertain"


def metrics(rows, predictions):
    if len(rows) != len(predictions) or any(p not in {"clear", "unclear", "uncertain"} for p in predictions):
        raise ValueError("预测数量或状态不合法")
    pairs = [(row["label"], prediction) for row, prediction in zip(rows, predictions) if row["label"] in (1, 2)]
    counts = {str(label): {state: sum(l == label and p == state for l, p in pairs)
                          for state in ("clear", "unclear", "uncertain")} for label in (1, 2)}
    clear_total, blurry_total = sum(counts["1"].values()), sum(counts["2"].values())
    total = len(pairs)
    correct = counts["1"]["clear"] + counts["2"]["unclear"]
    unknown = counts["1"]["uncertain"] + counts["2"]["uncertain"]
    ratio = lambda numerator, denominator: numerator / denominator if denominator else None
    evaluable = bool(clear_total and blurry_total)
    passed = (evaluable and counts["1"]["unclear"] == 0 and counts["2"]["clear"] / blurry_total <= 0.10
              and unknown / total <= 0.20 and correct / total >= 0.90)
    return dict(total=total, correct=correct, confusion=counts, human_skipped=len(rows) - total,
                clear_false_discard=counts["1"]["unclear"], blurry_false_pass=counts["2"]["clear"],
                unknown=unknown, clear_false_discard_rate=ratio(counts["1"]["unclear"], clear_total),
                blurry_false_pass_rate=ratio(counts["2"]["clear"], blurry_total), unknown_rate=ratio(unknown, total),
                accuracy=ratio(correct, total), calibration_floor_passed=bool(passed))


def best_fiqa_threshold(rows):
    values = sorted({row["fiqa"] for row in rows if row["label"] in (1, 2) and finite(row["fiqa"])})
    if not values:
        return None
    thresholds = [math.nextafter(values[0], -math.inf), *[(a + b) / 2 for a, b in zip(values, values[1:])],
                  math.nextafter(values[-1], math.inf)]
    candidates = []
    for threshold in thresholds:
        predictions = ["uncertain" if not finite(row["fiqa"]) else "clear" if row["fiqa"] >= threshold else "unclear" for row in rows]
        measured = metrics(rows, predictions)
        candidates.append(dict(threshold=threshold, metrics=measured, predictions=predictions))
    return min(candidates, key=lambda c: (-c["metrics"]["correct"], c["metrics"]["clear_false_discard"],
                                          c["metrics"]["blurry_false_pass"], c["threshold"]))
