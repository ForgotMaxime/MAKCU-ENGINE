"""Pure protocol helpers. No I/O, no serial import.

This module is the primary Rust-port candidate: every function is a
pure bytes/str transform covered by unit tests. All failures raise
MakcuProtocolError (validation) — never bare ValueError/IndexError.
"""

from .errors import MakcuProtocolError
from .types import Button, DeviceInfo, Position, Screen


def build_cmd(name: str, *args: int | str) -> bytes:
    body = f"{name}({','.join(str(a) for a in args)})"
    return body.encode("ascii") + b"\r"


def strip_prompt(raw: bytes) -> str:
    try:
        text = raw.decode("ascii", errors="strict")
    except UnicodeDecodeError as e:
        raise MakcuProtocolError(f"non-ascii reply: {raw!r}") from e
    if ">>>" not in text:
        raise MakcuProtocolError(f"missing prompt in {raw!r}")
    payload = text.split(">>>")[0]
    return payload.strip().replace("\r", "").replace("\n", "")


def parse_device(payload: str) -> DeviceInfo:
    parts = dict(p.split(":", 1) for p in payload.split(";") if ":" in p)
    return DeviceInfo(
        route=parts.get("R", ""),
        mouse_period=parts.get("M", ""),
        keyboard_period=parts.get("K", ""),
        controller_period=parts.get("C", ""),
    )


def _parse_pair(payload: str, what: str) -> tuple[int, int]:
    try:
        inner = payload[payload.index("(") + 1 : payload.index(")")]
        a, b = inner.split(",")
        return int(a.strip()), int(b.strip())
    except (ValueError, IndexError) as e:
        raise MakcuProtocolError(f"bad {what}: {payload!r}") from e


def parse_screen(payload: str) -> Screen:
    w, h = _parse_pair(payload, "screen")
    check_screen_size(w, h)
    return Screen(width=w, height=h)


def parse_getpos(payload: str) -> Position:
    x, y = _parse_pair(payload, "getpos")
    return Position(x=x, y=y)


def clamp_i16(v: int) -> int:
    if not -32768 <= v <= 32767:
        raise MakcuProtocolError(f"out of i16 range: {v}")
    return v


def check_xy(x: int, y: int) -> None:
    if not 0 <= x <= 32767:
        raise MakcuProtocolError(f"x out of range: {x}")
    if not 0 <= y <= 32767:
        raise MakcuProtocolError(f"y out of range: {y}")


def check_screen_size(w: int, h: int) -> None:
    if not 1 <= w <= 32767:
        raise MakcuProtocolError(f"width out of range: {w}")
    if not 1 <= h <= 32767:
        raise MakcuProtocolError(f"height out of range: {h}")


def check_mask_bit(v: int) -> None:
    if v not in (0, 1):
        raise MakcuProtocolError(f"mask must be 0/1: {v}")


def check_click(button_value: int, count: int, hold_ms: int | None) -> None:
    if not 1 <= button_value <= 5:
        raise MakcuProtocolError(f"button out of range: {button_value}")
    if not 1 <= count <= 255:
        raise MakcuProtocolError(f"count out of range: {count}")
    if hold_ms is not None and not 1 <= hold_ms <= 5000:
        raise MakcuProtocolError(f"hold_ms out of range: {hold_ms}")


def check_stream_mode(mode: int) -> None:
    if mode not in (1, 2, 3):
        raise MakcuProtocolError(f"stream mode out of range: {mode}")


def check_autorelease(timeout_ms: int) -> None:
    if not 0 <= timeout_ms <= 60000:
        raise MakcuProtocolError(f"autorelease out of range: {timeout_ms}")


def button_method(button: Button) -> str:
    return {
        Button.LEFT: "left",
        Button.RIGHT: "right",
        Button.MIDDLE: "middle",
        Button.SIDE1: "side1",
        Button.SIDE2: "side2",
    }[button]


def mask_method(button: Button) -> str:
    return f"{button_method(button)}_mask"


def parse_button_event(raw: bytes) -> int:
    """Parse one V4.036+ prefixed stream frame: b'km.' + mask + CRLF."""
    if len(raw) == 6 and raw[:3] == b"km." and raw.endswith(b"\r\n"):
        return raw[3]
    raise MakcuProtocolError(f"bad button frame: {raw!r}")
