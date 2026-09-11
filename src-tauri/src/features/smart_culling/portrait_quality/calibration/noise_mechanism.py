"""Prepare and audit known-signal diagnostics; never read photograph category labels."""

import argparse
import json
from pathlib import Path

from legacy import content_hash, utc_now
from prepare import write_new
from stress import FEATURE

FILES = ["portrait_quality.rs", "portrait_quality/linear_detail.rs", "portrait_quality/linear_response.rs",
    "portrait_quality/robust_detail.rs", "portrait_quality/stress_variants.rs", "portrait_quality/noise_mechanism.rs",
    "portrait_quality/mechanism_samples.rs", "portrait_quality/mechanism_energy.rs", "analysis.rs",
    "../../../Cargo.lock", "../../../Cargo.toml", "portrait_quality/calibration/noise_mechanism.py", "portrait_quality/calibration/noise_mechanism_report.py"]


def matrix():
    cases = []
    for panel in ["flat", "checker", "step1", "step3", "step6"]:
        for side in [128, 256, 512]:
            processing = ["unclipped_float", "rgb8", "rgb8_sharpen"]
            if side == 512:
                processing += ["rgb8_resize256", "rgb8_resize128"]
            for amplitude in [8, 24, 48]:
                for seed in [0x51455741, 1, 20260910]:
                    for operation in processing:
                        cases.append(dict(id=f"{panel}-{side}-{amplitude}-{seed}-{operation}",
                            panel=panel, side=side, amplitude=amplitude, seed=seed, processing=operation))
    return cases


def prepare(output):
    protocol = dict(schema="qraw-noise-mechanism-protocol-1", created_at=utc_now(), cases=matrix(),
        code_hashes={name: content_hash(FEATURE / name) for name in FILES},
        design_sha256=content_hash(output / "PROTOCOL.md"), expected_rows=495,
        panel_formula="flat=128; checker=((x/7+y/9)%2) ? 200 : 40; step=round(20+215*(1+tanh((x-(side-1)/2)/width))/2), width=1/3/6",
        noise_formula="existing LCG uniform independent RGB integers; same draws for float and RGB8",
        response_method="reuse 0.3 kernels, 15% inset, 3-pixel valid boundary, no threshold fitting",
        tolerances=dict(linearity_absolute=1e-8, energy_relative_to_max_one=1e-8),
        notes=["浮点行使用未裁切 RGB32f 经宿主 to_luma32f 转换再升 f64 测量；只有加噪参考；image::unsharpen 内部整数化和裁切，不能冒充未裁切浮点锐化。",
               "锐化与缩放行直接使用实际 RGB8 路径。浮点行 clipping fraction 是对应 RGB8 将发生的裁切比例。",
               "残差相对同样处理的干净图计算；同时保留处理前干净图能量，避免把锐化当成仅改变噪声。",
               "sigma 在完整输入上估计；真实残差方差也统计完整输入；滤波能量只用同一固定内 ROI。",
               "所有条件和种子都报告；已知原信号与真实残差仅为诊断工具，不能部署为单张照片算法。"],
        labels_used=False, new_candidate_created=False, runtime_enabled=False)
    write_new(output / "PROTOCOL.json", protocol)
    print(f"已固定 {len(protocol['cases'])} 个条件，尚未测量：{output}")


def checked_read(output):
    protocol = json.loads((output / "PROTOCOL.json").read_text())
    observed = json.loads((output / "observations.json").read_text())
    if (protocol["schema"] != "qraw-noise-mechanism-protocol-1"
            or observed["schema"] != "qraw-noise-mechanism-observations-1"
            or protocol["design_sha256"] != content_hash(output / "PROTOCOL.md")
            or set(protocol["code_hashes"]) != set(FILES)
            or protocol["cases"] != matrix() or observed["protocol_sha256"] != content_hash(output / "PROTOCOL.json")
            or observed["code_hashes"] != protocol["code_hashes"]
            or [row["case"] for row in observed["rows"]] != protocol["cases"]):
        raise ValueError("矩阵或观测与冻结协议不一致")
    for name, digest in protocol["code_hashes"].items():
        if content_hash(FEATURE / name) != digest:
            raise ValueError(f"测量代码变化：{name}")
    return protocol, observed["rows"]

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["prepare", "report"])
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if not args.output.is_absolute():
        parser.error("必须使用绝对路径")
    if args.action == "prepare":
        prepare(args.output)
    else:
        from noise_mechanism_report import report
        report(args.output)
