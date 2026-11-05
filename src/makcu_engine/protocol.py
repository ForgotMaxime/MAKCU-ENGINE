"""Pure protocol helpers. No I/O, no serial import.

This module is the primary Rust-port candidate: every function is a
pure bytes/str transform covered by unit tests.
"""

from .types import Button, DeviceInfo, Position, Screen


def build_cmd(name: str, *args: int | str) -> bytes:
    body = f"{name}({','.join(str(a) for a in args)})"
    return body.encode("ascii") + b"\r"


def strip_prompt(raw: bytes) -> str:
    text = raw.decode("ascii", errors="strict")
    if ">>>" not in text:
        raise ValueError(f"missing prompt in {raw!r}")
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


def parse_screen(payload: str) -> Screen:
    inner = payload[payload.index("(") + 1 : payload.index(")")]
    w, h = (int(v) for v in inner.split(","))
    return Screen(width=w, height=h)


def parse_getpos(payload: str) -> Position:
    inner = payload[payload.index("(") + 1 : payload.index(")")]
    x, y = (int(v) for v in inner.split(","))
    return Position(x=x, y=y)


def clamp_i16(v: int) -> int:
    if not -32768 <= v <= 32767:
        raise ValueError(f"out of i16 range: {v}")
    return v


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
    raise ValueError(f"bad button frame: {raw!r}")
