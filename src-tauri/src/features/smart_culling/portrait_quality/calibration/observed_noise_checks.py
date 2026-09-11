"""Independent reconstruction checks; estimator error is a result, not an I/O failure."""

import math

from candidate import finite
from decomposition_checks import relative_error
from observed_noise import RULES, VERSION


def verify(protocol, rows, prior):
    maximum = dict(replay=0.0, proxy_identity=0.0, blocks=0.0)
    def check(group, actual, expected):
        error = relative_error(actual, expected)
        if error > RULES["identity_tolerance"]:
            raise ValueError(f"{group} 校验失败：{error}")
        maximum[group] = max(maximum[group], error)
    for row, previous in zip(rows, prior):
        for variant, old in zip(row["variants"], previous["variants"]):
            check("replay", variant["crop_dimensions"], old["crop_dimensions"])
            check("replay", variant["cross_channel"], old["cross_channel"])
            estimate = variant["estimate"]
            if (estimate["version"] != VERSION or estimate["input"] != "observedRgb8Only"
                    or estimate["median_radius"] != 2 or estimate["block_grid"] != [4, 4]
                    or estimate["clean_reference_used"] is not False or estimate["confidence_interval_validated"] is not False
                    or len(estimate["scales"]) != 2):
                raise ValueError("估计器输入契约或尺度错误")
            dimensions = old["cross_channel"]["innerDimensions"]
            check("replay", estimate["inner_dimensions"], dimensions)
            width, height = [d - 6 for d in dimensions]
            if min(width, height) < 4:
                raise ValueError("响应窗口不足")
            counts = [0] * 16
            for y in range(height):
                for x in range(width):
                    counts[(y * 4 // height) * 4 + x * 4 // width] += 1
            for index, scale in enumerate(estimate["scales"]):
                proxy, blocks = scale["proxy_decomposition"], scale["sampling"]
                observed = old["cross_channel"]["scales"][index]["commonEnergy"]
                check("replay", proxy["observed"], observed)
                check("proxy_identity", [proxy["count"], proxy["order"]], [width * height, [2, 4][index]])
                pairs = proxy["pairs_rg_rb_gb"]
                if len(pairs) != 3:
                    raise ValueError("代理通道对缺失")
                for pair in pairs:
                    check("proxy_identity", pair["linearity_error"], 0.0)
                    check("proxy_identity", pair["observed_energy"], pair["signal_energy"] + pair["noise_energy"] + pair["cross_term"])
                for name, key in [("reference", "signal_energy"), ("residual_common", "noise_energy"),
                                  ("signal_noise_cross", "cross_term")]:
                    check("proxy_identity", proxy[name], sum(p[key] for p in pairs) / 3.0)
                for name in ("same_position", "different_position"):
                    if len(proxy[name + "_pairs"]) != 3:
                        raise ValueError("代理空间通道对缺失")
                    check("proxy_identity", proxy[name], sum(proxy[name + "_pairs"]) / 3.0)
                check("proxy_identity", proxy["residual_common"], proxy["same_position"] + proxy["different_position"])
                check("proxy_identity", observed, proxy["reference"] + proxy["residual_common"] + proxy["signal_noise_cross"])
                check("proxy_identity", scale["corrected_energy"], observed - proxy["residual_common"])
                if (blocks["counts"] != counts or len(blocks["means"]) != 16 or len(blocks["delete_means"]) != 16
                        or not all(finite(v) for v in blocks["means"] + blocks["delete_means"])
                        or not finite(blocks["standard_error"]) or blocks["standard_error"] < 0):
                    raise ValueError("分块统计缺失或非有限")
                total = sum(n * mean for n, mean in zip(counts, blocks["means"]))
                check("blocks", blocks["mean"], total / sum(counts))
                check("blocks", blocks["mean"], proxy["residual_common"])
                deleted = [(total - n * mean) / (sum(counts) - n) for n, mean in zip(counts, blocks["means"])]
                check("blocks", blocks["delete_means"], deleted)
                center = sum(deleted) / 16.0
                se = math.sqrt(15.0 / 16.0 * sum((value - center) ** 2 for value in deleted))
                check("blocks", blocks["standard_error"], se)
    return maximum
