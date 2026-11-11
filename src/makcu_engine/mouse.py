"""Mouse-only high-level API. All validation via protocol.check_*, framing in protocol."""

from __future__ import annotations

import time

from .abc import Transport
from .batch import BatchBuilder
from .errors import MakcuProtocolError
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
    """Mouse-only high-level API over any Transport."""
    def __init__(self, transport: Transport) -> None:
        self._t = transport
        self._stream_mode: int | None = None

    def version(self) -> str:
        """Firmware identity string (km.MAKCU on V4)."""
        return self._t.query("km.version")

    def device(self) -> DeviceInfo:
        """Route and report periods (R:MK;M:..;K:..;C:..)."""
        return parse_device(self._t.query("km.device"))

    def echo(self) -> int:
        """Command echo flag (0 = silent setters)."""
        return int(self._t.query("km.echo"))

    def set_echo(self, enabled: bool) -> None:
        """Enable or disable command echo."""
        self._t.set("km.echo", 1 if enabled else 0)

    def getpos(self) -> Position:
        """Tracked pointer position estimate."""
        return parse_getpos(self._t.query("km.getpos"))

    def screen(self) -> Screen:
        """Virtual screen size."""
        return parse_screen(self._t.query("km.screen"))

    def moving(self) -> bool:
        """True while injected movement is still being sent."""
        return self._t.query("km.moving") == "1"

    def wait_idle(self, timeout_s: float = 2.0, poll_s: float = 0.02) -> bool:
        """Block until no injected movement remains. True if idle, False on timeout."""
        deadline = time.monotonic() + timeout_s
        while True:
            if not self.moving():
                return True
            if time.monotonic() >= deadline:
                return False
            time.sleep(min(poll_s, max(0.0, deadline - time.monotonic())))

    def button_state(self, button: Button) -> ButtonOwnership:
        """Physical/injected ownership of a button."""
        return ButtonOwnership(int(self._t.query(f"km.{button_method(button)}")))

    def lock_state(self, target: LockTarget) -> bool:
        """Whether a lock target is currently locked."""
        return self._t.query(f"km.lock_{target.value}") == "1"

    def move(self, dx: int, dy: int) -> None:
        """Injected relative move (i16 each axis)."""
        self._t.set("km.move", clamp_i16(dx), clamp_i16(dy))

    def move_now(self, dx: int, dy: int) -> None:
        """Injected move requesting the next free report."""
        self._t.set("km.move_now", clamp_i16(dx), clamp_i16(dy))

    def moveto(self, x: int, y: int) -> None:
        """Absolute move on the virtual screen (0..=32767)."""
        check_xy(x, y)
        self._t.set("km.moveto", x, y)

    def set_screen(self, width: int, height: int, *, confirm: bool = False) -> None:
        """Change the virtual screen. Requires confirm=True: this moves the
        tracked pointer origin on a live system."""
        if not confirm:
            raise MakcuProtocolError("set_screen needs confirm=True (changes live tracking)")
        check_screen_size(width, height)
        self._t.set("km.screen", width, height)

    def wheel(self, delta: int) -> None:
        """Injected wheel delta (i16)."""
        self._t.set("km.wheel", clamp_i16(delta))

    def move_smooth(self, dx: int, dy: int, steps: int = 10, step_s: float = 0.005) -> None:
        """Split a move into steps so the firmware path looks drawn, not jumped."""
        if steps < 1:
            raise MakcuProtocolError(f"steps out of range: {steps}")
        if step_s < 0:
            raise MakcuProtocolError(f"step_s out of range: {step_s}")
        px = py = 0
        for i in range(1, steps + 1):
            x = round(dx * i / steps)
            y = round(dy * i / steps)
            self.move(x - px, y - py)
            px, py = x, y
            if step_s and i < steps:
                time.sleep(step_s)

    def drag(
        self, button: Button, dx: int, dy: int, steps: int = 10, step_s: float = 0.005
    ) -> None:
        """Press, smooth-move, release. The button always goes up again."""
        self.down(button)
        try:
            self.move_smooth(dx, dy, steps, step_s)
        finally:
            self.up(button)

    def silent(self, x: int, y: int) -> None:
        """Move, silent left click, move back."""
        check_xy(x, y)
        self._t.set("km.silent", x, y)

    def down(self, button: Button) -> None:
        """Press a mouse button."""
        self._t.set(f"km.{button_method(button)}", 1)

    def press(self, button: Button) -> None:
        """Press a mouse button (alias of down)."""
        self.down(button)

    def up(self, button: Button, silent_release: bool = False) -> None:
        """Release a mouse button; silent variant skips the frame."""
        self._t.set(f"km.{button_method(button)}", 2 if silent_release else 0)

    def release(self, button: Button, silent_release: bool = False) -> None:
        """Release a mouse button (alias of up)."""
        self.up(button, silent_release)

    def click(self, button: Button, count: int = 1, hold_ms: int | None = None) -> None:
        """Scheduled clicks with optional hold time."""
        check_click(button.value, count, hold_ms)
        if hold_ms is None:
            self._t.set("km.click", button.value, count)
        else:
            self._t.set("km.click", button.value, count, hold_ms)

    def double_click(self, button: Button, gap_s: float = 0.05) -> None:
        """Two clicks with one sleep between them."""
        if gap_s < 0:
            raise MakcuProtocolError(f"gap_s out of range: {gap_s}")
        self.click(button)
        if gap_s:
            time.sleep(gap_s)
        self.click(button)

    def init(self) -> None:
        """Best-effort release of all buttons; stop the button stream if running."""
        for button in (Button.LEFT, Button.RIGHT, Button.MIDDLE, Button.SIDE1, Button.SIDE2):
            try:
                self.up(button)
            except Exception:  # noqa: BLE001 — best-effort release never raises
                pass
        if self._stream_mode is not None:
            try:
                self.stop_button_stream()
            except Exception:  # noqa: BLE001 — best-effort release never raises
                pass

    def set_move_mask(self, left: int, right: int, down: int, up: int) -> None:
        """Block physical movement per direction (each 0/1)."""
        for v in (left, right, down, up):
            check_mask_bit(v)
        self._t.set("km.move_mask", left, right, down, up)

    def set_wheel_mask(self, down: int, up: int) -> None:
        """Block physical wheel per direction (each 0/1)."""
        check_mask_bit(down)
        check_mask_bit(up)
        self._t.set("km.wheel_mask", down, up)

    def set_button_mask(self, button: Button, enabled: bool) -> None:
        """Block a physical button; injection still works."""
        self._t.set(f"km.{mask_method(button)}", 1 if enabled else 0)

    def set_lock(self, target: LockTarget, enabled: bool) -> None:
        """Lock or unlock a button/axis target."""
        self._t.set(f"km.lock_{target.value}", 1 if enabled else 0)

    def move_ff(self, dx: int, dy: int) -> None:
        """Fire-and-forget relative move (no reply wait)."""
        self._t.fire_and_forget("km.move", clamp_i16(dx), clamp_i16(dy))

    def batch(self) -> BatchBuilder:
        """Start a coalesced BatchBuilder on this transport."""
        return BatchBuilder(self._t)

    # --- test-build features (V4.103+, absent from older firmware: ERR) ---
    def autorelease(self) -> int:
        """Injected-button autorelease timeout in ms (V4.103+)."""
        return int(self._t.query("km.autorelease"))

    def set_autorelease(self, timeout_ms: int) -> None:
        """Set the autorelease timeout, 0 disables (V4.103+)."""
        check_autorelease(timeout_ms)
        self._t.set("km.autorelease", timeout_ms)

    def heartbeat(self) -> None:
        """Restart the autorelease timer (V4.103+)."""
        self._t.set("km.heartbeat")

    def api_version(self) -> int:
        """Firmware API version integer (V4.105 reports 1)."""
        return int(self._t.query("km.api_version"))

    # --- button stream (no queries in flight while polling; loop lifts this) ---
    def start_button_stream(self, mode: int = 1) -> None:
        """Enable physical button-change streaming (modes 1-3)."""
        check_stream_mode(mode)
        self._t.set("km.buttons", mode)
        self._stream_mode = mode

    def button_stream_state(self) -> bool:
        """True while the button stream is enabled on the device."""
        return self._t.query("km.buttons").strip() == "1"

    def stop_button_stream(self) -> None:
        """Disable button-change streaming."""
        self._t.set("km.buttons", 0)
        self._stream_mode = None

    def poll_button_event(self, timeout_s: float = 0.5) -> int:
        """Next button mask; mode 3 reads one bare byte."""
        if self._stream_mode is None:
            raise MakcuProtocolError("stream not started — call start_button_stream first")
        if self._t.event_loop_running():
            if self._stream_mode == 3:
                raise MakcuProtocolError("mode 3 needs the legacy poll path (stop the event loop)")
            return parse_button_event(self._t.poll_event(timeout_s))
        if self._stream_mode == 3:
            return parse_button_event(self._t.read_exact(1, timeout_s))
        return parse_button_event(self._t.read_line(timeout_s))
