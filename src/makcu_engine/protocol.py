"""Pure protocol helpers. No I/O, no serial import.

This module is the primary Rust-port candidate: every function is a
pure bytes/str transform covered by unit tests. All failures raise
MakcuProtocolError (validation) — never bare ValueError/IndexError.
"""

from .errors import MakcuProtocolError
from .types import Button, DeviceInfo, Position, Screen


def build_cmd(name: str, *args: int | str) -> bytes:
    """Frame one ASCII command: name(a,b) plus carriage return."""
    body = f"{name}({','.join(str(a) for a in args)})"
    return body.encode("ascii") + b"\r"


def strip_prompt(raw: bytes) -> str:
    """Strip the prompt and a possible echo line; last line wins.

    Echo off: `<result>\\r\\n>>> ` -> result.
    Echo on: `<input>\\r\\n<result>\\r\\n>>> ` -> result.
    `km.version()` is special and never echoes.
    """
    try:
        text = raw.decode("ascii", errors="strict")
    except UnicodeDecodeError as e:
        raise MakcuProtocolError(f"non-ascii reply: {raw!r}") from e
    if ">>>" not in text:
        raise MakcuProtocolError(f"missing prompt in {raw!r}")
    payload = text.split(">>>")[0]
    lines = [line for line in payload.replace("\r", "\n").split("\n") if line.strip()]
    if not lines:
        raise MakcuProtocolError(f"empty reply: {raw!r}")
    return lines[-1].strip()


def parse_device(payload: str) -> DeviceInfo:
    """Parse an R:..;M:..;K:..;C:.. device string."""
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
    """Parse and range-check a km.screen(w,h) reply."""
    w, h = _parse_pair(payload, "screen")
    check_screen_size(w, h)
    return Screen(width=w, height=h)


def parse_getpos(payload: str) -> Position:
    """Parse a km.getpos(x,y) reply."""
    x, y = _parse_pair(payload, "getpos")
    return Position(x=x, y=y)


def clamp_i16(v: int) -> int:
    """Range-check an i16 command argument."""
    if not -32768 <= v <= 32767:
        raise MakcuProtocolError(f"out of i16 range: {v}")
    return v


def check_xy(x: int, y: int) -> None:
    """Range-check virtual-screen coordinates (0..=32767)."""
    if not 0 <= x <= 32767:
        raise MakcuProtocolError(f"x out of range: {x}")
    if not 0 <= y <= 32767:
        raise MakcuProtocolError(f"y out of range: {y}")


def check_screen_size(w: int, h: int) -> None:
    """Range-check virtual-screen dimensions (1..=32767)."""
    if not 1 <= w <= 32767:
        raise MakcuProtocolError(f"width out of range: {w}")
    if not 1 <= h <= 32767:
        raise MakcuProtocolError(f"height out of range: {h}")


def check_mask_bit(v: int) -> None:
    """Range-check a 0/1 mask argument."""
    if v not in (0, 1):
        raise MakcuProtocolError(f"mask must be 0/1: {v}")


def check_click(button_value: int, count: int, hold_ms: int | None) -> None:
    """Validate click button/count/hold arguments."""
    if not 1 <= button_value <= 5:
        raise MakcuProtocolError(f"button out of range: {button_value}")
    if not 1 <= count <= 255:
        raise MakcuProtocolError(f"count out of range: {count}")
    if hold_ms is not None and not 1 <= hold_ms <= 5000:
        raise MakcuProtocolError(f"hold_ms out of range: {hold_ms}")


def check_bezier(segments: int, ctrl_x: int, ctrl_y: int) -> None:
    """Validate firmware-side bezier args: segments 1..=255, control i16."""
    if not 1 <= segments <= 255:
        raise MakcuProtocolError(f"segments out of range: {segments}")
    if not -32768 <= ctrl_x <= 32767:
        raise MakcuProtocolError(f"ctrl_x out of i16 range: {ctrl_x}")
    if not -32768 <= ctrl_y <= 32767:
        raise MakcuProtocolError(f"ctrl_y out of i16 range: {ctrl_y}")


def check_stream_mode(mode: int) -> None:
    """Validate a button-stream mode (1..=3)."""
    if mode not in (1, 2, 3):
        raise MakcuProtocolError(f"stream mode out of range: {mode}")


def check_autorelease(timeout_ms: int) -> None:
    """Validate an autorelease timeout (0..=60000)."""
    if not 0 <= timeout_ms <= 60000:
        raise MakcuProtocolError(f"autorelease out of range: {timeout_ms}")


def button_method(button: Button) -> str:
    """Wire name for injection/query.

    Side buttons use the ms1/ms2 names (like the lock targets and the
    reference library); the firmware also accepts side1/side2.
    """
    return {
        Button.LEFT: "left",
        Button.RIGHT: "right",
        Button.MIDDLE: "middle",
        Button.SIDE1: "ms1",
        Button.SIDE2: "ms2",
    }[button]


def mask_method(button: Button) -> str:
    """Wire name for a button mask (left_mask, ...).

    Masks keep the side1/side2 names — the firmware documents no
    ms1_mask/ms2_mask variants.
    """
    return {
        Button.LEFT: "left_mask",
        Button.RIGHT: "right_mask",
        Button.MIDDLE: "middle_mask",
        Button.SIDE1: "side1_mask",
        Button.SIDE2: "side2_mask",
    }[button]


def parse_button_event(raw: bytes) -> int:
    """Parse a V4 button stream frame: bare mask byte (mode 3) or
    b'km.' + mask + CRLF (modes 1/2, V4.036+)."""
    if len(raw) == 1:
        return raw[0]
    if len(raw) == 6 and raw[:3] == b"km." and raw.endswith(b"\r\n"):
        return raw[3]
    raise MakcuProtocolError(f"bad button frame: {raw!r}")


def _is_stream_frame(line: bytes) -> bool:
    """Prefixed stream frame: b'km.' + mask (< 0x20) + CRLF.

    The < 0x20 threshold separates a mask byte from command echo text
    (letters are >= 0x61).
    """
    return len(line) == 6 and line[:3] == b"km." and line[3] < 0x20 and line.endswith(b"\r\n")


def split_frames(buf: bytes) -> tuple[list[bytes], list[bytes], bytes]:
    """Split a raw byte stream into complete frames.

    Returns (replies, events, rest). A reply runs through the end of the
    first `>>> ` prompt. A CRLF-terminated line *before* that prompt is an
    event only if it matches the stream frame shape — otherwise the whole
    reply (echo + prompt) is consumed as one unit, so interleaved
    `frame + reply` input demultiplexes correctly. Bare bytes < 0x20 with
    no terminator pending are mode-3 masks and go out as single-byte
    events (`\\r`/`\\n` alone are kept: they may complete a split CRLF).
    """
    replies: list[bytes] = []
    events: list[bytes] = []
    while True:
        prompt_at = buf.find(b">>> ")
        if prompt_at >= 0:
            head = buf[:prompt_at]
            crlf_at = head.find(b"\r\n")
            if crlf_at >= 0 and _is_stream_frame(head[: crlf_at + 2]):
                end = crlf_at + 2
                events.append(bytes(buf[:end]))
                buf = buf[end:]
                continue
            end = prompt_at + len(b">>> ")
            replies.append(bytes(buf[:end]))
            buf = buf[end:]
            continue
        crlf_at = buf.find(b"\r\n")
        if crlf_at >= 0:
            line = buf[: crlf_at + 2]
            if _is_stream_frame(line):
                events.append(bytes(line))
                buf = buf[crlf_at + 2 :]
                continue
            return replies, events, bytes(buf)
        if buf and buf[0] < 0x20 and buf[0:1] not in (b"\r", b"\n"):
            events.append(bytes(buf[:1]))
            buf = buf[1:]
            continue
        return replies, events, bytes(buf)
