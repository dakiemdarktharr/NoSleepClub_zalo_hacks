"""Replay a saved run with the official, unmodified SDK in a separate process.

This checks a submission's execution receipts, not its task accuracy. Inference
does not see this process and never gets to reset a session during a sample.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import re
import sys


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--split", required=True, choices=["train", "public_test", "private_test"])
    parser.add_argument("--predictions", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args()
    root = args.root.resolve(strict=True)
    sys.path.insert(0, str(root / args.split))
    from zaic_tools import ToolEnv

    seen = set()
    mismatches = []
    rejected = 0
    with args.predictions.open(encoding="utf-8") as source:
        for line in source:
            if not line.strip():
                continue
            item = json.loads(line)
            sample_id = item["id"]
            if not isinstance(sample_id, str) or re.fullmatch(r"[A-Za-z0-9_-]{1,128}", sample_id) is None or sample_id in seen:
                raise ValueError("invalid or duplicate sample id")
            if set(item) != {"id", "tool_calls", "state_hash"} or len(item["tool_calls"]) > 32:
                raise ValueError("invalid submission envelope")
            seen.add(sample_id)
            env = ToolEnv(args.split, sample_id, root=root)
            for call in item["tool_calls"]:
                if set(call) != {"name", "arguments"} or not isinstance(call["arguments"], dict):
                    raise ValueError("invalid submitted tool call")
                result = env.call(call["name"], call["arguments"])
                rejected += result.get("ok") is not True
            if env.get_tool_calls() != item["tool_calls"] or env.get_state_hash() != item["state_hash"]:
                mismatches.append(sample_id)
    report = {"scope": "separate official SDK replay; receipt integrity only, NOT task accuracy",
              "source": "challenge.zalo.ai", "split": args.split, "samples": len(seen),
              "replay_mismatches": len(mismatches), "mismatched_ids": mismatches,
              "schema_rejected_calls": rejected, "environment_files_read_directly": False,
              "passed": bool(seen) and not mismatches}
    with args.report.open("x", encoding="utf-8") as output:
        json.dump(report, output, ensure_ascii=False, indent=2)
    print(json.dumps({k: v for k, v in report.items() if k != "mismatched_ids"}, ensure_ascii=False))
    if not report["passed"]:
        raise SystemExit(2)


if __name__ == "__main__":
    main()
