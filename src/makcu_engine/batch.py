"""Coalesced command batch. One write, one ERR-window read.

Batch holds setters only: with echo off the firmware replies nothing on
success, so a whole batch costs a single error-window wait. Queries are
refused — their replies would interleave unpredictably.
"""

from __future__ import annotations

from .abc import Transport
from .protocol import build_cmd, button_method, clamp_i16
from .types import Button, LockTarget


class BatchBuilder:
    def __init__(self, transport: Transport) -> None:
        self._t = transport
        self._buf = bytearray()
        self._count = 0

    def __len__(self) -> int:
        return self._count

    def _add(self, name: str, *args: int | str) -> BatchBuilder:
        self._buf += build_cmd(name, *args)
        self._count += 1
        return self

    def move(self, dx: int, dy: int) -> BatchBuilder:
        return self._add("km.move", clamp_i16(dx), clamp_i16(dy))

    def move_now(self, dx: int, dy: int) -> BatchBuilder:
        return self._add("km.move_now", clamp_i16(dx), clamp_i16(dy))

    def wheel(self, delta: int) -> BatchBuilder:
        return self._add("km.wheel", clamp_i16(delta))

    def down(self, button: Button) -> BatchBuilder:
        return self._add(f"km.{button_method(button)}", 1)

    def up(self, button: Button) -> BatchBuilder:
        return self._add(f"km.{button_method(button)}", 0)

    def click(self, button: Button, count: int = 1, hold_ms: int | None = None) -> BatchBuilder:
        if hold_ms is None:
            return self._add("km.click", button.value, count)
        return self._add("km.click", button.value, count, hold_ms)

    def set_lock(self, target: LockTarget, enabled: bool) -> BatchBuilder:
        return self._add(f"km.lock_{target.value}", 1 if enabled else 0)

    def execute(self) -> int:
        n = len(self)
        if n:
            self._t.send_raw(bytes(self._buf))
        self._buf.clear()
        self._count = 0
        return n
