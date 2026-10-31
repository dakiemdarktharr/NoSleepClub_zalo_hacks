"""Evidence-gated notification decisions and a delivery-acknowledgement ledger.

No text-to-speech is performed. Production must authenticate event producers and
must not let an LLM certify its own `verified` field.
"""

import math
import sqlite3
from dataclasses import dataclass
from pathlib import Path
from typing import Literal

Kind = Literal[
    "goal.accepted", "worker.started", "sources.milestone", "finding.material",
    "permission.required", "input.required", "blocker.material", "goal.completed",
    "status.requested", "technical", "heartbeat",
]
KINDS = {
    "goal.accepted", "worker.started", "sources.milestone", "finding.material",
    "permission.required", "input.required", "blocker.material", "goal.completed",
    "status.requested", "technical", "heartbeat",
}
URGENT = {"permission.required", "input.required", "blocker.material", "goal.completed"}
EVIDENCE_REQUIRED = {
    "worker.started", "sources.milestone", "finding.material", "goal.completed",
}


@dataclass(frozen=True)
class Event:
    goal_id: str
    event_id: str
    kind: Kind
    occurred_at: float
    summary: str
    milestone_key: str = ""
    verified: bool = False
    evidence_refs: tuple[str, ...] = ()
    read_source_ids: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        if not isinstance(self.verified, bool):
            raise ValueError("verified must be a boolean supplied by a trusted verifier")
        if self.kind not in KINDS:
            raise ValueError("unknown event kind")
        if not self.goal_id or not self.event_id:
            raise ValueError("goal_id and event_id are required")
        if not math.isfinite(self.occurred_at) or self.occurred_at < 0:
            raise ValueError("occurred_at must be a finite nonnegative timestamp")
        if not isinstance(self.summary, str) or not self.summary.strip():
            raise ValueError("summary must be nonempty text")
        for refs in (self.evidence_refs, self.read_source_ids):
            if not all(isinstance(value, str) and value.strip() for value in refs):
                raise ValueError("evidence/source IDs must be nonempty strings")

    @property
    def notice_key(self) -> str:
        # Different event kinds may use the same milestone name without collision.
        return f"{self.kind}:{self.milestone_key or self.event_id}"


@dataclass(frozen=True)
class Context:
    now: float
    quiet: bool = False
    user_speaking: bool = False
    routine_cooldown_seconds: float = 45.0

    def __post_init__(self) -> None:
        if not isinstance(self.quiet, bool) or not isinstance(self.user_speaking, bool):
            raise ValueError("quiet and user_speaking must be booleans")
        if not math.isfinite(self.now) or self.now < 0:
            raise ValueError("now must be a finite nonnegative timestamp")
        if not math.isfinite(self.routine_cooldown_seconds) or self.routine_cooldown_seconds < 0:
            raise ValueError("cooldown must be finite and nonnegative")


@dataclass(frozen=True)
class Decision:
    surface: Literal["none", "timeline", "action_card"]
    speech: Literal["none", "ready", "defer"]
    reason: str
    text: str = ""
    notice_key: str = ""


class NotificationLedger:
    """Persist delivery acknowledgements, not raw reasoning or audio.

    Record only after a channel acknowledges delivery. If a crash happens after
    speech but before acknowledgement, speech can repeat; this is not an
    exactly-once audio delivery protocol. One local dispatcher should own writes.
    """

    def __init__(self, path: str | Path = ":memory:") -> None:
        self.connection = sqlite3.connect(str(path))
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS deliveries ("
            "goal_id TEXT NOT NULL, notice_key TEXT NOT NULL, "
            "channel TEXT NOT NULL CHECK(channel IN ('surface','speech')), "
            "delivered_at REAL NOT NULL, "
            "PRIMARY KEY(goal_id, notice_key, channel))"
        )
        self.connection.commit()

    def close(self) -> None:
        self.connection.close()

    def delivered(self, goal_id: str, notice_key: str, channel: str) -> bool:
        return self.connection.execute(
            "SELECT 1 FROM deliveries WHERE goal_id=? AND notice_key=? AND channel=?",
            (goal_id, notice_key, channel),
        ).fetchone() is not None

    def last_speech(self, goal_id: str) -> float | None:
        result = self.connection.execute(
            "SELECT MAX(delivered_at) FROM deliveries WHERE goal_id=? AND channel='speech'",
            (goal_id,),
        ).fetchone()
        return result[0] if result else None

    def acknowledge(self, goal_id: str, notice_key: str, channel: str, at: float) -> None:
        if channel not in {"surface", "speech"}:
            raise ValueError("unknown delivery channel")
        if not goal_id or not notice_key or not math.isfinite(at) or at < 0:
            raise ValueError("valid identifiers and delivery time are required")
        with self.connection:
            self.connection.execute(
                "INSERT OR IGNORE INTO deliveries VALUES (?, ?, ?, ?)",
                (goal_id, notice_key, channel, at),
            )


def decide(event: Event, context: Context, ledger: NotificationLedger) -> Decision:
    """Decide UI/audio independently; deferred speech is not marked delivered.

    The real dispatcher must re-evaluate deferred events against current goal
    state before speaking; old blockers may have resolved while queued.
    """
    if event.occurred_at > context.now:
        return Decision("none", "none", "future_event")
    if event.kind in {"technical", "heartbeat"}:
        return Decision("none", "none", "diagnostic_only")
    if event.kind in EVIDENCE_REQUIRED and not (event.verified and event.evidence_refs):
        return Decision("none", "none", "unverified_claim")

    text = event.summary.strip()
    if event.kind == "sources.milestone":
        count = len(set(event.read_source_ids))
        if count < 3:
            return Decision("none", "none", "below_source_milestone")
        # Render the count from IDs supplied by the trusted source registry.
        text = f"Đã đối chiếu {count} nguồn duy nhất có nội dung đã đọc."

    key = event.notice_key
    shown = ledger.delivered(event.goal_id, key, "surface")
    spoken = ledger.delivered(event.goal_id, key, "speech")
    surface = "none" if shown else (
        "action_card" if event.kind in {"permission.required", "input.required"} else "timeline"
    )
    if spoken:
        return Decision(surface, "none", "speech_already_delivered", text, key)
    if context.quiet and event.kind != "status.requested":
        return Decision(surface, "none", "quiet_mode", text, key)
    if context.user_speaking:
        return Decision(surface, "defer", "user_has_floor", text, key)
    last = ledger.last_speech(event.goal_id)
    if (
        event.kind not in URGENT and event.kind != "status.requested"
        and last is not None and context.now - last < context.routine_cooldown_seconds
    ):
        return Decision(surface, "defer", "routine_cooldown", text, key)
    return Decision(surface, "ready", "meaningful_event", text, key)
