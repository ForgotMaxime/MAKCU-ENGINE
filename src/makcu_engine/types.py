"""Mouse-only domain types. No I/O here — portable to Rust as-is."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from enum import Enum, IntEnum


class Button(IntEnum):
    """Physical mouse buttons (1-5, as used by km.click)."""
    LEFT = 1
    RIGHT = 2
    MIDDLE = 3
    SIDE1 = 4
    SIDE2 = 5
    # V4 aliases: km.ms1/ms2 address the same side buttons (lock targets
    # are named ml/mm/mr/ms1/ms2). Same values, canonical names stay SIDE*.
    MS1 = 4
    MS2 = 5


class ButtonOwnership(IntEnum):
    """Physical vs injected button ownership (km.left & co)."""
    NONE = 0
    PHYSICAL = 1
    INJECTED = 2
    BOTH = 3


@dataclass(frozen=True)
class DeviceInfo:
    """Route and report periods from km.device()."""
    route: str
    mouse_period: str
    keyboard_period: str
    controller_period: str


@dataclass(frozen=True)
class Screen:
    """Virtual screen size from km.screen()."""
    width: int
    height: int


@dataclass(frozen=True)
class Position:
    """Tracked pointer position from km.getpos()."""
    x: int
    y: int


@dataclass(frozen=True)
class Monitor:
    """One display in virtual-desktop coordinates.

    x/y is the monitor origin (negative when placed left/above primary),
    w/h its size in physical pixels, scale the DPI factor
    (physical pixels per logical unit).
    """
    x: int
    y: int
    w: int
    h: int
    scale: float = 1.0


@dataclass(frozen=True)
class Bounds:
    """Bounding box over monitors: origin + size in logical units."""
    x: int
    y: int
    w: int
    h: int


def virtual_bounds(monitors: Sequence[Monitor]) -> Bounds:
    """Bounding box spanning all monitors; negative origins supported."""
    if not monitors:
        raise ValueError("virtual_bounds needs at least one monitor")
    left = min(m.x for m in monitors)
    top = min(m.y for m in monitors)
    right = max(m.x + m.w / m.scale for m in monitors)
    bottom = max(m.y + m.h / m.scale for m in monitors)
    return Bounds(x=left, y=top, w=int(round(right - left)), h=int(round(bottom - top)))


def to_virtual(x: int, y: int, monitor: Monitor) -> Position:
    """Monitor-local physical pixel -> virtual-desktop logical point."""
    return Position(
        x=monitor.x + int(round(x / monitor.scale)),
        y=monitor.y + int(round(y / monitor.scale)),
    )


def to_local(x: int, y: int, monitor: Monitor) -> Position:
    """Virtual-desktop logical point -> monitor-local physical pixel."""
    return Position(
        x=int(round((x - monitor.x) * monitor.scale)),
        y=int(round((y - monitor.y) * monitor.scale)),
    )


def remap_point(x: int, y: int, src: Monitor, dst: Monitor) -> Position:
    """Re-express a src-local pixel as a dst-local pixel via virtual space."""
    v = to_virtual(x, y, src)
    return to_local(v.x, v.y, dst)


class LockTarget(str, Enum):
    """Lockable buttons and axes (km.lock_* targets)."""
    ML = "ml"
    MR = "mr"
    MM = "mm"
    MS1 = "ms1"
    MS2 = "ms2"
    MX = "mx"
    MY = "my"
    MW = "mw"
    MX_POS = "mx+"
    MX_NEG = "mx-"
    MY_POS = "my+"
    MY_NEG = "my-"
    MW_POS = "mw+"
    MW_NEG = "mw-"
