"""Execution complexity and authorization are independent dimensions."""

from dataclasses import dataclass
from enum import StrEnum


class Mode(StrEnum):
    ANSWER = "answer"
    TOOL = "bounded_tool"
    GOAL = "durable_goal"
    UNSUPPORTED = "unsupported"


class Gate(StrEnum):
    READY = "ready"
    INPUT = "waiting_input"
    PERMISSION = "waiting_permission"
    APPROVAL = "waiting_approval"


@dataclass(frozen=True)
class TaskFacts:
    requires_tools: bool = False
    tool_supported: bool = True
    expected_steps: int = 0
    has_dependencies: bool = False
    requires_resume: bool = False
    open_ended_discovery: bool = False
    missing_parameters: tuple[str, ...] = ()
    critical_slot_ambiguous: bool = False
    needs_new_grant: bool = False
    needs_effect_approval: bool = False

    def __post_init__(self) -> None:
        for field in ("requires_tools", "tool_supported", "has_dependencies", "requires_resume",
                      "open_ended_discovery", "critical_slot_ambiguous", "needs_new_grant",
                      "needs_effect_approval"):
            if not isinstance(getattr(self, field), bool):
                raise ValueError(f"{field} must be a boolean")
        if isinstance(self.expected_steps, bool) or not isinstance(self.expected_steps, int):
            raise ValueError("expected_steps must be an integer")
        if self.expected_steps < 0:
            raise ValueError("expected_steps cannot be negative")
        if not all(isinstance(name, str) and name.strip() for name in self.missing_parameters):
            raise ValueError("missing_parameters must contain nonempty strings")


@dataclass(frozen=True)
class Route:
    mode: Mode
    gate: Gate
    reason: str


def route(facts: TaskFacts) -> Route:
    """Route trusted structured facts, not arbitrary speech or a task-length heuristic.

    This chooses a mode and the first gate. Every future effect still needs broker
    validation, including effects following an initially READY route.
    """
    if facts.requires_tools and not facts.tool_supported:
        return Route(Mode.UNSUPPORTED, Gate.READY, "no_supported_tool")
    complex_task = (
        facts.requires_resume or facts.has_dependencies
        or facts.open_ended_discovery or facts.expected_steps >= 4
    )
    mode = Mode.GOAL if complex_task else (
        Mode.TOOL if facts.requires_tools else Mode.ANSWER
    )
    if facts.missing_parameters or facts.critical_slot_ambiguous:
        return Route(mode, Gate.INPUT, "missing_or_ambiguous_input")
    if facts.needs_new_grant:
        return Route(mode, Gate.PERMISSION, "new_capability_required")
    if facts.needs_effect_approval:
        return Route(mode, Gate.APPROVAL, "exact_effect_preview_required")
    return Route(mode, Gate.READY, "bounded" if not complex_task else "durability_required")
