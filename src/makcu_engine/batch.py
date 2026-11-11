"""Coalesced command batch. One write, one ERR-window read.

Batch holds setters only: with echo off the firmware replies nothing on
success, so a whole batch costs a single error-window wait. Queries are
refused — their replies would interleave unpredictably.

Covers the same setter set as Mouse, with the same validation.
"""

from __future__ import annotations

from .abc import Transport
from .errors import MakcuProtocolError
from .protocol import (
    build_cmd,
    button_method,
    check_autorelease,
    check_click,
    check_mask_bit,
    check_xy,
    clamp_i16,
    mask_method,
)
from .types import Button, LockTarget


class BatchBuilder:
    """Coalesced setter batch: one write, one ERR-window read."""
    def __init__(self, transport: Transport) -> None:
        self._t = transport
        self._buf = bytearray()
        self._names: list[str] = []
        self._count = 0

    def __len__(self) -> int:
        return self._count

    def _add(self, name: str, *args: int | str) -> BatchBuilder:
        """Append one framed command."""
        self._buf += build_cmd(name, *args)
        self._names.append(name)
        self._count += 1
        return self

    def move(self, dx: int, dy: int) -> BatchBuilder:
        """Queue a relative move."""
        return self._add("km.move", clamp_i16(dx), clamp_i16(dy))

    def move_now(self, dx: int, dy: int) -> BatchBuilder:
        """Queue an immediate relative move."""
        return self._add("km.move_now", clamp_i16(dx), clamp_i16(dy))

    def moveto(self, x: int, y: int) -> BatchBuilder:
        """Queue an absolute move on the virtual screen."""
        check_xy(x, y)
        return self._add("km.moveto", x, y)

    def silent(self, x: int, y: int) -> BatchBuilder:
        """Queue a move, silent left click, move back."""
        check_xy(x, y)
        return self._add("km.silent", x, y)

    def wheel(self, delta: int) -> BatchBuilder:
        """Queue a wheel step."""
        return self._add("km.wheel", clamp_i16(delta))

    def down(self, button: Button) -> BatchBuilder:
        """Queue a button press."""
        return self._add(f"km.{button_method(button)}", 1)

    def up(self, button: Button, silent_release: bool = False) -> BatchBuilder:
        """Queue a button release; silent variant wires a 2 like Mouse.up."""
        return self._add(f"km.{button_method(button)}", 2 if silent_release else 0)

    def click(self, button: Button, count: int = 1, hold_ms: int | None = None) -> BatchBuilder:
        """Queue a scheduled click; validated like Mouse.click."""
        check_click(button.value, count, hold_ms)
        if hold_ms is None:
            return self._add("km.click", button.value, count)
        return self._add("km.click", button.value, count, hold_ms)

    def set_move_mask(self, left: int, right: int, down: int, up: int) -> BatchBuilder:
        """Queue a physical move-direction mask (each 0/1)."""
        for v in (left, right, down, up):
            check_mask_bit(v)
        return self._add("km.move_mask", left, right, down, up)

    def set_wheel_mask(self, down: int, up: int) -> BatchBuilder:
        """Queue a physical wheel-direction mask (each 0/1)."""
        check_mask_bit(down)
        check_mask_bit(up)
        return self._add("km.wheel_mask", down, up)

    def set_button_mask(self, button: Button, enabled: bool) -> BatchBuilder:
        """Queue a physical button mask."""
        return self._add(f"km.{mask_method(button)}", 1 if enabled else 0)

    def set_lock(self, target: LockTarget, enabled: bool) -> BatchBuilder:
        """Queue an input lock on a button/axis target."""
        return self._add(f"km.lock_{target.value}", 1 if enabled else 0)

    def set_autorelease(self, timeout_ms: int) -> BatchBuilder:
        """Queue an autorelease timeout in ms (V4.103+, test builds)."""
        check_autorelease(timeout_ms)
        return self._add("km.autorelease", timeout_ms)

    def heartbeat(self) -> BatchBuilder:
        """Queue an autorelease keep-alive (no reply)."""
        return self._add("km.heartbeat")

    def execute(self) -> int:
        """Flush as one write + one ERR-window read; returns flushed count."""
        n = len(self)
        if n:
            names = ",".join(self._names)
            try:
                self._t.send_raw(bytes(self._buf))
            except MakcuProtocolError as e:
                raise MakcuProtocolError(f"batch [{names}] failed: {e}") from e
        self._buf.clear()
        self._names.clear()
        self._count = 0
        return n
