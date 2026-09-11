"""Fixed error metrics, clean leakage controls, and three-seed variation diagnostics."""

import json
from collections import defaultdict
from statistics import mean, stdev

from decomposition_report import statistics
from observed_noise import RULES
from paired_degradation import SEEDS

TERMS = ("same_position", "different_position", "residual_common")


def analyze(protocol, rows, prior):
    conditions, seed_groups = [], defaultdict(list)
    for row, previous in zip(rows, prior):
        for case, variant, old in zip(protocol["cases"], row["variants"], previous["variants"]):
            noise = next((s for s in case["stages"] if s["kind"] in ("noise", "shared_noise")), None)
            order = ">".join(s["kind"] for s in case["stages"])
            scales = []
            for estimate, actual in zip(variant["estimate"]["scales"], old["decomposition"]["scales"]):
                proxy = estimate["proxy_decomposition"]
                errors = {term: proxy[term] - actual[term] for term in TERMS}
                errors["corrected_energy"] = estimate["corrected_energy"] - actual["reference"]
                relative = {term: abs(errors[term]) / max(1.0, abs(actual[term])) for term in TERMS}
                relative["corrected_energy"] = abs(errors["corrected_energy"]) / max(1.0, abs(actual["reference"]))
                scales.append(dict(estimated={term: proxy[term] for term in TERMS},
                    actual={term: actual[term] for term in (*TERMS, "signal_noise_cross", "reference", "observed")},
                    errors=errors, relative_errors=relative, corrected_energy=estimate["corrected_energy"],
                    block_standard_error=estimate["sampling"]["standard_error"],
                    within_two_block_se=abs(errors["residual_common"]) <= RULES["sensitivity_multiplier"] * estimate["sampling"]["standard_error"],
                    original_drift=abs(actual["observed"] - actual["reference"]) / max(1.0, abs(actual["reference"]))))
            stable = {term: max(s["relative_errors"][term] for s in scales) <= RULES["relative_error_limit"]
                      for term in (*TERMS, "corrected_energy")}
            record = dict(sample_id=row["sample_id"], image_ref=row["image_ref"], case_id=case["id"],
                reference_id=case["reference_id"], seed=case["seed"], noise=noise["kind"] if noise else "clean",
                amplitude=noise["value"] if noise else 0, order=order, scales=scales, stable=stable,
                original_stable=max(s["original_drift"] for s in scales) <= RULES["relative_error_limit"],
                within_two_block_se=all(s["within_two_block_se"] for s in scales),
                negative_original=any(s["actual"]["observed"] < 0 for s in scales),
                negative_estimate=any(s["estimated"]["residual_common"] < 0 for s in scales),
                negative_corrected=any(s["corrected_energy"] < 0 for s in scales))
            conditions.append(record)
            if noise:
                seed_groups[(row["sample_id"], json.dumps(case["stages"], sort_keys=True))].append(record)
    groups = defaultdict(list)
    for record in conditions:
        kind = record["noise"]
        for group in (kind, f"{kind}/amplitude{record['amplitude']}/{record['order']}",
                      f"{kind}/seed{record['seed']}", f"{kind}/{record['image_ref']}"):
            groups[group].append(record)
    summaries = {}
    for group, records in groups.items():
        summaries[group] = dict(count=len(records), stable={term: sum(r["stable"][term] for r in records)
            for term in (*TERMS, "corrected_energy")}, original_stable=sum(r["original_stable"] for r in records),
            within_two_block_se=sum(r["within_two_block_se"] for r in records),
            negative_original=sum(r["negative_original"] for r in records),
            negative_estimate=sum(r["negative_estimate"] for r in records),
            negative_corrected=sum(r["negative_corrected"] for r in records),
            errors={f"scale{i}/{term}": statistics([r["scales"][i]["errors"][term] for r in records])
                    for i in range(2) for term in (*TERMS, "corrected_energy")})
    variation = []
    for records in seed_groups.values():
        if len(records) != len(SEEDS) or {r["seed"] for r in records} != set(SEEDS):
            raise ValueError("种子重复或缺失，不能计算采样诊断")
        variation.append(dict(sample_id=records[0]["sample_id"], case_ids=[r["case_id"] for r in records],
            scales=[{term: dict(error_mean=mean(r["scales"][i]["errors"][term] for r in records),
                error_sd=stdev(r["scales"][i]["errors"][term] for r in records),
                actual_sd=stdev(r["scales"][i]["actual"][term] for r in records),
                estimated_sd=stdev(r["scales"][i]["estimated"][term] for r in records)) for term in TERMS} for i in range(2)]))
    clean = summaries["clean"]
    clean_passed = clean["count"] > 0 and clean["stable"]["corrected_energy"] / clean["count"] >= RULES["clean_required_fraction"]
    screening = clean_passed and all(summaries[kind]["count"] > 0 and all(
        count / summaries[kind]["count"] >= RULES["minimum_fraction"] for count in summaries[kind]["stable"].values())
        for kind in ("noise", "shared_noise"))
    return dict(conditions=conditions, groups=summaries, seed_variation=variation,
        clean_leakage_passed=clean_passed, screening_passed=screening, candidate_fitted=False,
        classification_evaluable=False, confidence_interval_validated=False, release_ready=False)
