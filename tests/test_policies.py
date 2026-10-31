import tempfile
import unittest
import json
import os
import subprocess
import sys
from pathlib import Path

from nora_policy import Context, Event, Gate, Mode, NotificationLedger, TaskFacts, decide, route


class CommandLineScenarios(unittest.TestCase):
    def test_vietnamese_replay_works_when_windows_pipe_defaults_to_cp1252(self):
        root = Path(__file__).resolve().parents[1]
        result = subprocess.run(
            [sys.executable, "-m", "nora_policy", "replay", "examples/progress-events.jsonl"],
            cwd=root, env={**os.environ, "PYTHONIOENCODING": "cp1252"},
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            encoding="utf-8", check=True, timeout=10,
        )
        rows = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual(len(rows), 9)
        self.assertIn("Tôi sẽ", rows[0]["text"])
        self.assertEqual(rows[1]["reason"], "diagnostic_only")
        self.assertEqual(rows[6]["reason"], "unverified_claim")


class RoutingScenarios(unittest.TestCase):
    def test_short_dangerous_task_still_requires_approval(self):
        result = route(TaskFacts(requires_tools=True, expected_steps=1, needs_effect_approval=True))
        self.assertEqual((result.mode, result.gate), (Mode.TOOL, Gate.APPROVAL))

    def test_one_prompt_can_be_a_durable_goal(self):
        result = route(TaskFacts(requires_tools=True, has_dependencies=True, requires_resume=True))
        self.assertEqual((result.mode, result.gate), (Mode.GOAL, Gate.READY))

    def test_missing_permission_preserves_goal_mode(self):
        result = route(TaskFacts(requires_tools=True, has_dependencies=True, needs_new_grant=True))
        self.assertEqual((result.mode, result.gate), (Mode.GOAL, Gate.PERMISSION))

    def test_ambiguous_amount_requests_input_before_approval(self):
        result = route(TaskFacts(requires_tools=True, critical_slot_ambiguous=True, needs_effect_approval=True))
        self.assertEqual(result.gate, Gate.INPUT)

    def test_unavailable_app_never_becomes_an_invented_tool(self):
        result = route(TaskFacts(requires_tools=True, tool_supported=False))
        self.assertEqual(result.mode, Mode.UNSUPPORTED)

    def test_string_boolean_does_not_silently_change_routing(self):
        with self.assertRaises(ValueError):
            TaskFacts(requires_tools="false")


class NotificationScenarios(unittest.TestCase):
    def setUp(self):
        self.ledger = NotificationLedger()

    def tearDown(self):
        self.ledger.close()

    def event(self, kind="finding.material", **changes):
        defaults = dict(goal_id="g", event_id="e", kind=kind, occurred_at=100,
                        summary="Một phát hiện có ích.", verified=True,
                        evidence_refs=("receipt:1",))
        defaults.update(changes)
        return Event(**defaults)

    def test_training_noise_and_unchanged_heartbeat_do_not_interrupt(self):
        for kind in ("technical", "heartbeat"):
            result = decide(self.event(kind), Context(now=100), self.ledger)
            self.assertEqual((result.surface, result.speech), ("none", "none"))

    def test_completion_needs_verifier_and_evidence(self):
        for changes in ({"verified": False}, {"evidence_refs": ()}):
            result = decide(self.event("goal.completed", **changes), Context(now=100), self.ledger)
            self.assertEqual(result.reason, "unverified_claim")

    def test_sources_are_counted_from_unique_read_ids(self):
        event = self.event("sources.milestone", summary="Tôi đã đọc 999 nguồn!",
                           read_source_ids=("a", "b", "a", "c"))
        result = decide(event, Context(now=100), self.ledger)
        self.assertIn("3 nguồn", result.text)
        self.assertNotIn("999", result.text)

    def test_search_hits_without_enough_read_sources_are_silent(self):
        event = self.event("sources.milestone", read_source_ids=("same", "same"))
        self.assertEqual(decide(event, Context(now=100), self.ledger).surface, "none")

    def test_quiet_mode_retains_permission_card(self):
        result = decide(self.event("permission.required"), Context(now=100, quiet=True), self.ledger)
        self.assertEqual((result.surface, result.speech), ("action_card", "none"))

    def test_user_speech_defers_even_completion_without_acknowledging_it(self):
        event = self.event("goal.completed")
        result = decide(event, Context(now=100, user_speaking=True), self.ledger)
        self.assertEqual(result.speech, "defer")
        self.assertFalse(self.ledger.delivered("g", event.notice_key, "speech"))
        self.assertEqual(decide(event, Context(now=110), self.ledger).speech, "ready")

    def test_status_question_is_answered_in_quiet_mode(self):
        event = self.event("status.requested")
        result = decide(event, Context(now=100, quiet=True), self.ledger)
        self.assertEqual(result.speech, "ready")

    def test_routine_cooldown_does_not_hold_back_material_blocker(self):
        self.ledger.acknowledge("g", "old", "speech", 95)
        routine = decide(self.event(), Context(now=100), self.ledger)
        blocker = decide(self.event("blocker.material"), Context(now=100), self.ledger)
        self.assertEqual(routine.speech, "defer")
        self.assertEqual(blocker.speech, "ready")

    def test_channel_acknowledgements_are_separate(self):
        event = self.event()
        self.ledger.acknowledge("g", event.notice_key, "surface", 100)
        result = decide(event, Context(now=100), self.ledger)
        self.assertEqual((result.surface, result.speech), ("none", "ready"))

    def test_deduplication_is_scoped_to_goal(self):
        event = self.event()
        self.ledger.acknowledge("different-goal", event.notice_key, "speech", 100)
        self.assertEqual(decide(event, Context(now=100), self.ledger).speech, "ready")

    def test_delivery_is_not_repeated_after_restart_or_duplicate_event_id(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "notifications.sqlite3"
            first = NotificationLedger(path)
            event = self.event(milestone_key="license-finding")
            decision = decide(event, Context(now=100), first)
            first.acknowledge("g", decision.notice_key, "surface", 100)
            first.acknowledge("g", decision.notice_key, "speech", 100)
            first.close()
            reopened = NotificationLedger(path)
            try:
                retried = self.event(event_id="different-event-id", milestone_key="license-finding")
                result = decide(retried, Context(now=150), reopened)
                self.assertEqual((result.surface, result.speech), ("none", "none"))
            finally:
                reopened.close()

    def test_unknown_event_or_invalid_time_is_rejected(self):
        for changes in ({"kind": "raw.reasoning"}, {"occurred_at": float("nan")}, {"occurred_at": -1}):
            with self.assertRaises(ValueError):
                self.event(**changes)

    def test_string_verification_cannot_certify_completion(self):
        with self.assertRaises(ValueError):
            self.event("goal.completed", verified="false")

    def test_string_quiet_preference_is_rejected(self):
        with self.assertRaises(ValueError):
            Context(now=100, quiet="false")


if __name__ == "__main__":
    unittest.main()
