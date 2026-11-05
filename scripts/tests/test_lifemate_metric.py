"""Original toy cases, with no competition inputs or model calls."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("lifemate_metric", Path(__file__).parents[1] / "evaluate_lifemate_train.py")
metric = importlib.util.module_from_spec(spec)
spec.loader.exec_module(metric)


class MetricTests(unittest.TestCase):
    def test_wrong_state_and_execution_failure_force_zero(self):
        call = {"name": "toy.set", "arguments": {"x": 1}}
        pred = {"tool_calls": [call], "state_hash": "a"}
        gold = {"tool_calls": [call], "state_hash": "b"}
        self.assertEqual(metric.sample_score(pred, gold, {}, [True])["score"], 0)
        gold["state_hash"] = "a"
        self.assertEqual(metric.sample_score(pred, gold, {}, [True])["score"], 1)
        self.assertEqual(metric.sample_score(pred, gold, {}, [True], failed=True)["score"], 0)

    def test_extra_and_rejected_calls_are_penalised(self):
        call = {"name": "toy.set", "arguments": {"x": 1}}
        pred = {"tool_calls": [call, call], "state_hash": "a"}
        gold = {"tool_calls": [call], "state_hash": "a"}
        self.assertAlmostEqual(metric.sample_score(pred, gold, {}, [True, False])["score"], 2 / 3)

    def test_fuzzy_requires_strictly_greater_than_point_six(self):
        self.assertEqual(metric.token_f1("a b", "a b c d e"), 4 / 7)
        self.assertEqual(metric.token_f1("a b c", "a b c d e f g"), 0.6)
        schema = {"toy.note": {"parameters": {"properties": {"text": {"x-match": "fuzzy"}}}}}
        self.assertFalse(metric.call_matches({"name": "toy.note", "arguments": {"text": "a b c"}},
                                             {"name": "toy.note", "arguments": {"text": "a b c d e f g"}}, schema))

    def test_maximum_matching_handles_overlapping_fuzzy_candidates(self):
        schema = {"toy.note": {"parameters": {"properties": {"text": {"x-match": "fuzzy"}}}}}
        call = lambda text: {"name": "toy.note", "arguments": {"text": text}}
        pred = [call("a b c"), call("a b")]
        gold = [call("a b"), call("b c")]
        self.assertEqual(metric.matched_calls(pred, gold, schema, [True, True]), 2)

    def test_empty_gold_accepts_only_empty_predictions(self):
        gold = {"tool_calls": [], "state_hash": "a"}
        self.assertEqual(metric.sample_score({"tool_calls": [], "state_hash": "a"}, gold, {}, [])["score"], 1)
        self.assertEqual(metric.sample_score({"tool_calls": [{"name": "toy.x", "arguments": {}}], "state_hash": "a"}, gold, {}, [True])["score"], 0)

    def test_a_perfect_partial_run_cannot_claim_complete_partition_coverage(self):
        coverage = metric.partition_coverage({"toy_0", "toy_1", "toy_2"}, {"toy_0"}, "all")
        self.assertFalse(coverage["complete_partition"])
        self.assertEqual(coverage["missing_samples"], 2)
        self.assertEqual(coverage["partition_coverage"], 1/3)
        self.assertFalse(metric.partition_coverage(set(), set(), "all")["complete_partition"])

    def test_partition_is_independent_of_labels_and_catches_cross_partition_outputs(self):
        ids = {f"toy_{index}" for index in range(12)}
        # Same independent vectors as the Rust runner, not a real-data fixture.
        held = {sample_id for sample_id in ids if metric.in_partition(sample_id, "holdout")}
        self.assertEqual(held, {"toy_6"})
        self.assertTrue(metric.partition_coverage(ids, ids-held, "development")["complete_partition"])
        self.assertEqual(metric.partition_coverage(ids, ids, "development")["outside_partition_samples"],1)


if __name__ == "__main__":
    unittest.main()
