"""JSON macro loader + relative-delta replay (example only).

Macro file format::

    {"version": 1, "screenLayout": {"w": 1920, "h": 1080},
     "events": [{"t_ms": 0, "x": 100, "y": 200}, ...]}

Helpers are pure (no I/O): ``dedupe_moves``, ``scale_xy``,
``adjust_delays``, ``to_rel_deltas``. Replay uses relative ``km.move``
deltas only and never changes persistent settings. By default prints
the plan; ``--replay`` executes it on an explicit ``--port``.
"""

from __future__ import annotations

import argparse
import json
import time
from pathlib import Path
from typing import Any, TypedDict


class Event(TypedDict):
    """One macro event: timestamp plus virtual-screen coordinates."""

    t_ms: int
    x: int
    y: int


def load_macro(path: str | Path) -> dict[str, Any]:
    """Load and validate a macro JSON file."""
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError("macro root must be an object")
    events = data.get("events")
    if not isinstance(events, list) or not events:
        raise ValueError("macro needs a non-empty events list")
    for e in events:
        if not isinstance(e, dict):
            raise ValueError(f"bad event: {e!r}")
        for k in ("t_ms", "x", "y"):
            if not isinstance(e.get(k), int) or isinstance(e.get(k), bool):
                raise ValueError(f"event {e!r} missing int {k}")
        if e["t_ms"] < 0:
            raise ValueError(f"negative t_ms: {e!r}")
        if not 0 <= e["x"] <= 32767:
            raise ValueError(f"x out of range 0..32767: {e!r}")
        if not 0 <= e["y"] <= 32767:
            raise ValueError(f"y out of range 0..32767: {e!r}")
    data["events"] = sorted(events, key=lambda e: int(e["t_ms"]))
    return data


def dedupe_moves(events: list[Event]) -> list[Event]:
    """Drop consecutive events with identical x/y (keep the first)."""
    out: list[Event] = []
    for e in events:
        if out and out[-1]["x"] == e["x"] and out[-1]["y"] == e["y"]:
            continue
        out.append(dict(e))
    return out


def scale_xy(events: list[Event], sx: float, sy: float) -> list[Event]:
    """Scale x/y by sx/sy (factors must be > 0)."""
    if sx <= 0 or sy <= 0:
        raise ValueError(f"scale factors must be > 0: {sx}, {sy}")
    return [
        {"t_ms": e["t_ms"], "x": round(e["x"] * sx), "y": round(e["y"] * sy)} for e in events
    ]


def adjust_delays(events: list[Event], factor: float) -> list[Event]:
    """Scale inter-event delays by factor (0.25..4); t_ms origin kept at 0."""
    if not 0.25 <= factor <= 4.0:
        raise ValueError(f"factor out of range 0.25-4: {factor}")
    if not events:
        return []
    base = events[0]["t_ms"]
    return [
        {
            "t_ms": round((e["t_ms"] - base) * factor),
            "x": e["x"],
            "y": e["y"],
        }
        for e in events
    ]


def to_rel_deltas(events: list[Event]) -> list[tuple[int, int, int]]:
    """Convert absolute events to [(dx, dy, wait_ms)] relative deltas."""
    if not events:
        raise ValueError("to_rel_deltas needs a non-empty events list")
    out: list[tuple[int, int, int]] = []
    px, py, pt = events[0]["x"], events[0]["y"], events[0]["t_ms"]
    out.append((0, 0, 0))
    for e in events[1:]:
        out.append((e["x"] - px, e["y"] - py, max(0, e["t_ms"] - pt)))
        px, py, pt = e["x"], e["y"], e["t_ms"]
    return out


def build_plan(events: list[Event]) -> list[tuple[int, int, int]]:
    """Dedupe then convert to relative deltas (pure, no I/O)."""
    return to_rel_deltas(dedupe_moves(events))


def format_plan(plan: list[tuple[int, int, int]]) -> str:
    """Render a plan for dry-run printing."""
    lines = [f"{i}: move({dx},{dy}) wait={w}ms" for i, (dx, dy, w) in enumerate(plan)]
    return "\n".join(lines)


def build_parser() -> argparse.ArgumentParser:
    """Build the macros CLI parser."""
    p = argparse.ArgumentParser(description="JSON macro plan/replay (example only)")
    p.add_argument("macro", help="Path to macro JSON file")
    p.add_argument("--replay", action="store_true", help="Execute on hardware")
    p.add_argument("--port", default=None, help="Serial port (required with --replay)")
    p.add_argument("--baud", type=int, default=4000000, help="Baud rate")
    p.add_argument("--scale", default=None, help="sx,sy factors, e.g. 0.5,0.5")
    p.add_argument("--speed", type=float, default=1.0, help="Delay factor 0.25-4")
    return p


def main(argv: list[str] | None = None) -> int:
    """Print the plan, or replay it in relative deltas with --replay."""
    args = build_parser().parse_args(argv)
    data = load_macro(args.macro)
    events: list[Event] = [dict(e) for e in data["events"]]
    if args.scale is not None:
        try:
            sx_s, sy_s = args.scale.split(",", 1)
            events = scale_xy(events, float(sx_s), float(sy_s))
        except ValueError as e:
            print(f"bad --scale: {e}")
            return 2
    try:
        events = adjust_delays(events, args.speed)
    except ValueError as e:
        print(f"bad --speed: {e}")
        return 2
    plan = build_plan(events)
    if not args.replay:
        print(format_plan(plan))
        return 0
    if not args.port:
        print("--replay requires --port")
        return 2
    from makcu_engine import MakcuTransport, Mouse

    with MakcuTransport(args.port, args.baud) as t:
        mouse = Mouse(t)
        for dx, dy, wait_ms in plan:
            if wait_ms:
                time.sleep(wait_ms / 1000.0)
            if dx or dy:
                mouse.move(dx, dy)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
