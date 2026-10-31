"""Replay synthetic events. Output is a simulation, never actual TTS delivery."""

import argparse
import json
import sys
from dataclasses import asdict
from pathlib import Path

from .notifications import Context, Event, NotificationLedger, decide


def main() -> None:
    # Windows redirected stdout may default to cp1252; JSON text is UTF-8.
    sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["replay"])
    parser.add_argument("input", type=Path)
    args = parser.parse_args()
    ledger = NotificationLedger()
    try:
        for line_number, line in enumerate(args.input.read_text(encoding="utf-8").splitlines(), 1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
                event_data = row["event"]
                for field in ("evidence_refs", "read_source_ids"):
                    event_data[field] = tuple(event_data.get(field, ()))
                event = Event(**event_data)
                context = Context(**row.get("context", {"now": event.occurred_at}))
                decision = decide(event, context, ledger)
            except (KeyError, TypeError, ValueError) as error:
                parser.exit(2, f"Invalid input line {line_number}: {error}\n")
            print(json.dumps({"simulation": True, "event_id": event.event_id, **asdict(decision)}, ensure_ascii=False))
            # Simulate successful dispatch acknowledgements; there is no speaker.
            if decision.surface != "none":
                ledger.acknowledge(event.goal_id, decision.notice_key, "surface", context.now)
            if decision.speech == "ready":
                ledger.acknowledge(event.goal_id, decision.notice_key, "speech", context.now)
    finally:
        ledger.close()


if __name__ == "__main__":
    main()
