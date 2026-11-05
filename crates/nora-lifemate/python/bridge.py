"""Thin, local adapter to the organisers' unchanged split-specific ToolEnv.

This process has no model, dataset-label reader, state inspection operation,
network transport or filesystem tool. Only ToolEnv opens the environment data.
The Rust supervisor owns all planning, ASR, deadlines and submission output.
"""
from __future__ import annotations

import json
from pathlib import Path
import sys

MAX_BYTES = 2_097_152


def main() -> None:
    if len(sys.argv) != 3 or sys.argv[2] not in {"train", "public_test", "private_test"}:
        raise ValueError("expected data root and split")
    root = Path(sys.argv[1]).resolve(strict=True)
    split = sys.argv[2]
    sys.path.insert(0, str(root / split))
    from zaic_tools import ToolEnv

    env = None
    seen: set[str] = set()
    finished = False
    while True:
        raw = sys.stdin.buffer.readline(MAX_BYTES + 1)
        if not raw:
            break
        if len(raw) > MAX_BYTES:
            raise ValueError("request exceeds protocol bound")
        message = json.loads(raw)
        operation = message.get("op")
        if operation == "begin":
            sample_id = message["id"]
            if not isinstance(sample_id, str) or len(sample_id) > 128 or not sample_id or any(
                not (c.isascii() and (c.isalnum() or c in "_-")) for c in sample_id
            ):
                raise ValueError("invalid sample id")
            if sample_id in seen or len(seen) >= 10_000:
                raise ValueError("a sample cannot be reset or opened twice")
            if env is not None and not finished:
                raise ValueError("finish the current sample before beginning another")
            seen.add(sample_id)
            env = ToolEnv(split, sample_id, root=root)
            finished = False
            response = {"ready": True}
        elif operation == "call" and env is not None and not finished:
            name, arguments = message["name"], message["arguments"]
            if not isinstance(name, str) or not isinstance(arguments, dict):
                raise ValueError("invalid call envelope")
            validation = env.validate(name, arguments)
            response = {"executed": False, "validation": validation}
            if validation.get("ok") is True:
                response["result"] = env.call(name, arguments)
                response["executed"] = True
        elif operation == "finish" and env is not None and not finished:
            response = {"tool_calls": env.get_tool_calls(), "state_hash": env.get_state_hash()}
            finished = True
        else:
            raise ValueError("operation unavailable in this protocol state")
        encoded = json.dumps(response, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded) > MAX_BYTES:
            raise ValueError("response exceeds protocol bound")
        sys.stdout.buffer.write(encoded + b"\n")
        sys.stdout.buffer.flush()


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        # Inputs and app data are never echoed into a transport diagnostic.
        print(type(error).__name__ + ": local environment bridge failed", file=sys.stderr)
        raise SystemExit(2)
