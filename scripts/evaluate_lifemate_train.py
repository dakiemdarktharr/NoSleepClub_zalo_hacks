"""Local mirror of the published metric, restricted to labelled TRAIN data.

The archive does not ship the organiser's scoring server. This script reports
a local train result, never an official public/private leaderboard score.
Gold receipts must come from a separate oracle-replay setup, not inference.
"""
from __future__ import annotations
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import sys
import unicodedata


def in_partition(sample_id: str, partition: str) -> bool:
    if partition not in {"development", "holdout", "all"}:
        raise ValueError("invalid train partition")
    held = int.from_bytes(hashlib.sha256(("NORA-LIFEMATE-HOLDOUT-v1:" + sample_id).encode()).digest(), "big") % 5 == 0
    return partition == "all" or held == (partition == "holdout")


def partition_coverage(dataset_ids: set[str], predicted_ids: set[str], partition: str) -> dict:
    expected = {sample_id for sample_id in dataset_ids if in_partition(sample_id, partition)}
    return {"partition": partition, "expected_samples": len(expected),
            "missing_samples": len(expected - predicted_ids),
            "outside_partition_samples": len(predicted_ids - expected),
            "partition_coverage": len(expected & predicted_ids) / len(expected) if expected else 0.0,
            "complete_partition": bool(expected) and predicted_ids == expected}


def token_f1(left: str, right: str) -> float:
    # The page specifies lower-case + NFC, but not punctuation tokenisation.
    # Use whitespace tokens, disclose this choice, and never call the mirror
    # the official evaluator. Exact calls are unaffected by that ambiguity.
    a = Counter(unicodedata.normalize("NFC", left.lower()).split())
    b = Counter(unicodedata.normalize("NFC", right.lower()).split())
    if not a and not b:
        return 1.0
    overlap = sum((a & b).values())
    return 2 * overlap / (sum(a.values()) + sum(b.values())) if overlap else 0.0


def call_matches(predicted: dict, gold: dict, specifications: dict) -> bool:
    if predicted["name"] != gold["name"]:
        return False
    a, b = predicted["arguments"], gold["arguments"]
    if not isinstance(a, dict) or not isinstance(b, dict) or a.keys() != b.keys():
        return False
    properties = specifications.get(gold["name"], {}).get("parameters", {}).get("properties", {})
    for key in a:
        if properties.get(key, {}).get("x-match") == "fuzzy":
            if not isinstance(a[key], str) or not isinstance(b[key], str) or token_f1(a[key], b[key]) <= 0.6:
                return False
        elif a[key] != b[key]:
            return False
    return True


def matched_calls(predicted: list, gold: list, specifications: dict, valid: list[bool]) -> int:
    # Maximum bipartite matching, not a greedy match that loses to duplicates
    # and fuzzy-overlap cases. One predicted call can match at most one gold.
    edges = [[j for j, call in enumerate(gold) if allowed and call_matches(p, call, specifications)]
             for p, allowed in zip(predicted, valid)]
    assigned: dict[int, int] = {}
    def augment(i: int, seen: set[int]) -> bool:
        for j in edges[i]:
            if j in seen:
                continue
            seen.add(j)
            if j not in assigned or augment(assigned[j], seen):
                assigned[j] = i
                return True
        return False
    return sum(augment(i, set()) for i in range(len(predicted)))


def sample_score(predicted: dict, gold: dict, specifications: dict, valid: list[bool], failed=False) -> dict:
    pc, gc = predicted["tool_calls"], gold["tool_calls"]
    matches = matched_calls(pc, gc, specifications, valid)
    state_match = predicted.get("state_hash") == gold.get("state_hash")
    tool_f1 = (2 * matches / (len(pc) + len(gc))) if pc or gc else 1.0
    score = float(not pc) if not gc else (tool_f1 if state_match and matches else 0.0)
    if failed:
        score = 0.0
    return {"score": score, "state_match": state_match, "tool_f1": tool_f1, "matches": matches,
            "gold_calls": len(gc), "predicted_calls": len(pc), "failed_or_timeout": failed,
            "exact_call_sequence": pc == gc}


def load_jsonl(path: Path) -> dict:
    result = {}
    for line in path.open(encoding="utf-8"):
        if not line.strip():
            continue
        item = json.loads(line)
        if item["id"] in result:
            raise ValueError("duplicate sample id")
        result[item["id"]] = item
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--gold-receipts", type=Path, required=True)
    parser.add_argument("--predictions", type=Path, required=True)
    parser.add_argument("--metrics", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--details", type=Path, required=True)
    parser.add_argument("--partition", choices=["development", "holdout", "all"], default="development")
    parser.add_argument("--require-complete-perfect", action="store_true",
                        help="fail unless the entire declared partition has score/state 100%% and no invalid/failed calls; still not an official score")
    args = parser.parse_args()
    root = args.root.resolve(strict=True)
    sys.path.insert(0, str(root / "train"))
    from zaic_tools.validate import schema_error

    dataset = load_jsonl(root / "train/train.jsonl")
    gold = load_jsonl(args.gold_receipts)
    predictions = load_jsonl(args.predictions)
    metrics = load_jsonl(args.metrics)
    catalog = json.loads((root / "train/train_tools_catalog.json").read_text(encoding="utf-8"))
    specifications = {t["name"]: t for a in catalog["apps"] for t in a["tools"]}
    if not predictions or not predictions.keys() <= dataset.keys() or metrics.keys() != predictions.keys():
        raise ValueError("prediction/metric coverage mismatch")
    coverage = partition_coverage(set(dataset), set(predictions), args.partition)
    if coverage["outside_partition_samples"]:
        raise ValueError("predictions cross the declared train partition")
    if not predictions.keys() <= gold.keys():
        raise ValueError("gold receipt coverage mismatch")
    results = []
    with args.details.open("x", encoding="utf-8") as output:
        for sample_id, prediction in predictions.items():
            item = dataset[sample_id]
            allowed = {name for app in item["context"]["installed_apps"].values() for name in app["tools"]} | {"sys.ask_missing_info"}
            valid = [schema_error(c["name"], c["arguments"], specifications, allowed) is None for c in prediction["tool_calls"]]
            metric = metrics[sample_id]
            result = sample_score(prediction, gold[sample_id], specifications, valid,
                                  metric["status"] != "complete" or metric["elapsed_ms"] > 30_000)
            result["id"] = sample_id
            result["gold_tool_names"] = [c["name"] for c in gold[sample_id]["tool_calls"]]
            result["predicted_tool_names"] = [c["name"] for c in prediction["tool_calls"]]
            result["schema_invalid_calls"] = valid.count(False)
            results.append(result)
            output.write(json.dumps(result, ensure_ascii=False) + "\n")
    report = {"scope": "local train metric mirror; NOT official leaderboard score", "source": "challenge.zalo.ai",
              "samples": len(results), "total_train_samples": len(dataset), "coverage": len(results) / len(dataset),
              "mean_score_percent": 100 * sum(r["score"] for r in results) / len(results),
              "perfect_samples": sum(r["score"] == 1 for r in results),
              "state_matches": sum(r["state_match"] for r in results),
              "exact_call_sequences": sum(r["exact_call_sequence"] for r in results),
              "failed_or_timeout": sum(r["failed_or_timeout"] for r in results),
              "schema_invalid_calls": sum(r["schema_invalid_calls"] for r in results),
              "fuzzy_tokenisation": "Unicode NFC + lower + whitespace; organiser punctuation rules not provided",
              "all_train_evaluated": len(predictions) == len(dataset),
              **coverage,
              "complete_perfect_local_partition": coverage["complete_partition"] and all(
                  r["score"] == 1 and r["state_match"] and not r["schema_invalid_calls"] for r in results),
              "replay_verified": False,
              "note": "A separate replay-verification pass is required before trusting submitted state hashes."}
    args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False))
    if args.require_complete_perfect and not report["complete_perfect_local_partition"]:
        raise SystemExit(2)


if __name__ == "__main__":
    main()
