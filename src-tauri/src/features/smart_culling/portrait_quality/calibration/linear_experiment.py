"""Create-new 0.3 experiment. Reuses frozen-input audit, matrix, fitter and reporting checks."""

import argparse
import json
from pathlib import Path

import gap_experiment
import linear_candidate as classifier
from linear_candidate import METHOD, VERSION
from robust_io import CODE_FILES, prepare_protocol, read_experiment

FILES = list(dict.fromkeys([*CODE_FILES, "portrait_quality.rs", "../../adjustment_utils.rs",
    "../../../Cargo.lock", "../../../Cargo.toml", "portrait_quality/linear_detail.rs",
    "portrait_quality/linear_detail_tests.rs", "portrait_quality/linear_response.rs", "infrastructure/clarity_input_tests.rs",
    *[f"portrait_quality/calibration/{name}" for name in ["linear_candidate.py", "linear_experiment.py",
        "linear_report.py", "linear_protocol.json", "test_linear_candidate.py", "gap_experiment.py"]]]))


def read(output):
    protocol, rows = read_experiment(output, candidate_version=VERSION,
                                    observation_schema="qraw-clarity-stress-observations-3")
    if protocol.get("method") != METHOD or protocol.get("measurement") != METHOD["measurement"]:
        raise ValueError("测量方法与冻结协议不一致")
    return protocol, rows


def prepare(frozen, output, phase, **extra):
    return prepare_protocol(frozen, output, phase, candidate_version=VERSION,
        measurement=METHOD["measurement"], code_files=FILES, notes=METHOD["notes"],
        protocol_version="qraw-clarity-linear-noise-stress-3", method=METHOD, **extra)


def fit_baseline(output):
    result = gap_experiment.fit_baseline(output, read, classifier)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return result["fit_status"] == "fitted"


def prepare_stress(baseline, output):
    candidate = gap_experiment.prepare_stress(baseline, output, read, prepare)
    print(f"候选状态 {candidate['fit_status']}；原 24×16 条件已固定：{output}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["baseline", "fit", "stress", "report"])
    parser.add_argument("--frozen", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if any(path is not None and not path.is_absolute() for path in [args.frozen, args.baseline, args.output]):
        parser.error("所有路径必须是绝对路径")
    if args.action == "baseline":
        if args.frozen is None:
            parser.error("需要冻结批次路径")
        prepare(args.frozen, args.output, "baseline")
        print(f"0.3 方法与代码已固定，尚未测量或拟合：{args.output}")
    elif args.action == "fit":
        raise SystemExit(0 if fit_baseline(args.output) else 1)
    elif args.action == "stress":
        if args.baseline is None:
            parser.error("需要原图拟合目录")
        prepare_stress(args.baseline, args.output)
    else:
        from linear_report import report
        report(args.output)
