"""Create-new 0.4 protocol and one-fit workflow; reuses versioned experiment utilities."""

import argparse
import json
from pathlib import Path

import edge_candidate as classifier
from edge_candidate import METHOD, VERSION
from gap_experiment import fit_baseline, prepare_stress
from robust_io import CODE_FILES, prepare_protocol, read_experiment

FILES = list(dict.fromkeys([*CODE_FILES, "portrait_quality.rs", "../../adjustment_utils.rs",
    "../../../Cargo.lock", "../../../Cargo.toml", "portrait_quality/linear_response.rs",
    *[f"portrait_quality/{name}.rs" for name in ["edge_profile", "edge_spread", "edge_spread_tests"]],
    *[f"portrait_quality/calibration/{name}" for name in ["edge_candidate.py", "edge_experiment.py",
        "edge_report.py", "edge_protocol.json", "test_edge_candidate.py", "gap_experiment.py"]]]))


def read(output):
    protocol, rows = read_experiment(output, candidate_version=VERSION,
                                    observation_schema="qraw-clarity-stress-observations-4")
    if protocol.get("method") != METHOD or protocol.get("measurement") != METHOD["measurement"]:
        raise ValueError("测量方法与冻结协议不一致")
    return protocol, rows


def prepare(frozen, output, phase, **extra):
    return prepare_protocol(frozen, output, phase, candidate_version=VERSION,
        measurement=METHOD["measurement"], code_files=FILES, notes=METHOD["notes"],
        protocol_version="qraw-clarity-edge-spread-stress-4", method=METHOD, **extra)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["baseline", "fit", "stress", "report"])
    parser.add_argument("--frozen", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if any(p is not None and not p.is_absolute() for p in [args.frozen, args.baseline, args.output]):
        parser.error("必须使用绝对路径")
    if args.action == "baseline":
        if args.frozen is None:
            parser.error("需要冻结批次")
        prepare(args.frozen, args.output, "baseline")
        print(f"0.4 方法和代码已固定；尚未测量或拟合：{args.output}")
    elif args.action == "fit":
        result = fit_baseline(args.output, read, classifier)
        print(json.dumps(result, ensure_ascii=False, indent=2))
        raise SystemExit(0 if result["fit_status"] == "fitted" else 1)
    elif args.action == "stress":
        if args.baseline is None:
            parser.error("需要基线目录")
        candidate = prepare_stress(args.baseline, args.output, read, prepare)
        print(f"已固定 24×16 条件，拟合状态：{candidate['fit_status']}")
    else:
        from edge_report import report
        report(args.output)
