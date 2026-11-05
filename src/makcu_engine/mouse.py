"""Mouse-only high-level API. All validation via protocol.check_*, framing in protocol."""

from __future__ import annotations

from .abc import Transport
from .batch import BatchBuilder
from .protocol import (
    button_method,
    check_autorelease,
    check_click,
    check_mask_bit,
    check_screen_size,
    check_stream_mode,
    check_xy,
    clamp_i16,
    mask_method,
    parse_button_event,
    parse_device,
    parse_getpos,
    parse_screen,
)
from .types import Button, ButtonOwnership, DeviceInfo, LockTarget, Position, Screen


class Mouse:
    def __init__(self, transport: Transport) -> None:
        self._t = transport

    def version(self) -> str:
        return self._t.query("km.version")

    def device(self) -> DeviceInfo:
        return parse_device(self._t.query("km.device"))

    def echo(self) -> int:
        return int(self._t.query("km.echo"))

    def set_echo(self, enabled: bool) -> None:
        self._t.set("km.echo", 1 if enabled else 0)

    def getpos(self) -> Position:
        return parse_getpos(self._t.query("km.getpos"))

    def screen(self) -> Screen:
        return parse_screen(self._t.query("km.screen"))

    def moving(self) -> bool:
        return self._t.query("km.moving") == "1"

    def button_state(self, button: Button) -> ButtonOwnership:
        return ButtonOwnership(int(self._t.query(f"km.{button_method(button)}")))

    def lock_state(self, target: LockTarget) -> bool:
        return self._t.query(f"km.lock_{target.value}") == "1"

    def move(self, dx: int, dy: int) -> None:
        self._t.set("km.move", clamp_i16(dx), clamp_i16(dy))

    def move_now(self, dx: int, dy: int) -> None:
        self._t.set("km.move_now", clamp_i16(dx), clamp_i16(dy))

    def moveto(self, x: int, y: int) -> None:
        check_xy(x, y)
        self._t.set("km.moveto", x, y)

    def set_screen(self, width: int, height: int) -> None:
        check_screen_size(width, height)
        self._t.set("km.screen", width, height)

    def wheel(self, delta: int) -> None:
        self._t.set("km.wheel", clamp_i16(delta))

    def silent(self, x: int, y: int) -> None:
        check_xy(x, y)
        self._t.set("km.silent", x, y)

    def down(self, button: Button) -> None:
        self._t.set(f"km.{button_method(button)}", 1)

    def up(self, button: Button, silent_release: bool = False) -> None:
        self._t.set(f"km.{button_method(button)}", 2 if silent_release else 0)

    def click(self, button: Button, count: int = 1, hold_ms: int | None = None) -> None:
        check_click(button.value, count, hold_ms)
        if hold_ms is None:
            self._t.set("km.click", button.value, count)
        else:
            self._t.set("km.click", button.value, count, hold_ms)

    def set_move_mask(self, left: int, right: int, down: int, up: int) -> None:
        for v in (left, right, down, up):
            check_mask_bit(v)
        self._t.set("km.move_mask", left, right, down, up)

    def set_wheel_mask(self, down: int, up: int) -> None:
        check_mask_bit(down)
        check_mask_bit(up)
        self._t.set("km.wheel_mask", down, up)

    def set_button_mask(self, button: Button, enabled: bool) -> None:
        self._t.set(f"km.{mask_method(button)}", 1 if enabled else 0)

    def set_lock(self, target: LockTarget, enabled: bool) -> None:
        self._t.set(f"km.lock_{target.value}", 1 if enabled else 0)

    def move_ff(self, dx: int, dy: int) -> None:
        self._t.fire_and_forget("km.move", clamp_i16(dx), clamp_i16(dy))

    def batch(self) -> BatchBuilder:
        return BatchBuilder(self._t)

    # --- test-build features (V4.103+, absent from older firmware: ERR) ---
    def autorelease(self) -> int:
        return int(self._t.query("km.autorelease"))

    def set_autorelease(self, timeout_ms: int) -> None:
        check_autorelease(timeout_ms)
        self._t.set("km.autorelease", timeout_ms)

    def heartbeat(self) -> None:
        self._t.set("km.heartbeat")

    def api_version(self) -> int:
        return int(self._t.query("km.api_version"))

    # --- button stream (no queries in flight while polling) ---
    def start_button_stream(self, mode: int = 1) -> None:
        check_stream_mode(mode)
        self._t.set("km.buttons", mode)

    def stop_button_stream(self) -> None:
        self._t.set("km.buttons", 0)

    def poll_button_event(self, timeout_s: float = 0.5) -> int:
        return parse_button_event(self._t.read_line(timeout_s))
