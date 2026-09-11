"""Validate paired identities independently of the Rust-reported error fields."""

from candidate import finite

TOLERANCE = 1e-8
TERM_NAMES = ("injected_noise", "noise_clip", "processing", "sharpen_clip")


def relative_error(actual, expected):
    if finite(actual) and finite(expected):
        return abs(actual - expected) / max(1.0, abs(expected))
    if isinstance(actual, dict) and isinstance(expected, dict) and actual.keys() == expected.keys():
        return max((relative_error(actual[key], expected[key]) for key in actual), default=0.0)
    if isinstance(actual, list) and isinstance(expected, list) and len(actual) == len(expected):
        return max((relative_error(a, b) for a, b in zip(actual, expected)), default=0.0)
    if type(actual) is type(expected) and actual == expected and type(actual) not in (float, int):
        return 0.0
    raise ValueError("观测结构或数值无效")


def verify_numerics(protocol, rows, previous):
    errors = dict(replay=0.0, linearity=0.0, pair_energy=0.0, spatial_energy=0.0, ordered_energy=0.0, reference=0.0)
    def check(name, actual, expected, scale=None):
        error = relative_error(actual, expected) if scale is None else abs(actual - expected) / scale
        if not finite(error) or error > TOLERANCE:
            raise ValueError(f"{name} 数值回归/恒等式失败：{error}")
        errors[name] = max(errors[name], error)
    for row, old in zip(rows, previous):
        references = {v["case_id"]: v for v in old["variants"]}
        for case, variant, prior in zip(protocol["cases"], row["variants"], old["variants"]):
            check("replay", variant["crop_dimensions"], prior["crop_dimensions"])
            check("replay", variant["cross_channel"], prior["cross_channel"])
            scales, trace = variant["decomposition"]["scales"], variant["trace"]
            if len(scales) != 2 or set(trace["terms"]) != set(TERM_NAMES):
                raise ValueError("分解尺度或处理项缺失")
            for index, scale in enumerate(scales):
                reference = references[case["reference_id"]]["cross_channel"]["scales"][index]["commonEnergy"]
                observed = prior["cross_channel"]["scales"][index]["commonEnergy"]
                unit = max(1.0, abs(observed), abs(reference))
                check("reference", [scale["reference"], scale["observed"]], [reference, observed])
                check("reference", [trace["reference"][index], trace["observed"][index]], [reference, observed])
                pairs = scale["pairs_rg_rb_gb"]
                if len(pairs) != 3 or len(scale["same_position_pairs"]) != 3 or len(scale["different_position_pairs"]) != 3:
                    raise ValueError("通道对缺失")
                for pair in pairs:
                    pair_unit = max(1.0, abs(pair["observed_energy"]), abs(pair["signal_energy"]))
                    check("linearity", pair["linearity_error"], 0.0)
                    check("pair_energy", pair["observed_energy"], pair["signal_energy"] + pair["noise_energy"] + pair["cross_term"], pair_unit)
                check("spatial_energy", scale["residual_common"], sum(p["noise_energy"] for p in pairs) / 3.0, unit)
                check("spatial_energy", scale["signal_noise_cross"], sum(p["cross_term"] for p in pairs) / 3.0, unit)
                for name in ("same_position", "different_position"):
                    check("spatial_energy", scale[name], sum(scale[name + "_pairs"]) / 3.0, unit)
                check("spatial_energy", observed, reference + scale["same_position"] + scale["different_position"] + scale["signal_noise_cross"], unit)
                delta = observed - reference
                check("ordered_energy", trace["actual_delta"][index], delta, unit)
                check("ordered_energy", sum(trace["terms"][name][index] for name in TERM_NAMES), delta, unit)
                if case["id"] == case["reference_id"]:
                    check("ordered_energy", [trace["terms"][name][index] for name in TERM_NAMES], [0.0] * 4)
                    check("spatial_energy", [scale["residual_common"], scale["signal_noise_cross"]], [0.0, 0.0])
    return errors
