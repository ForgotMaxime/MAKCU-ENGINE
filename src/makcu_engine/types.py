"""Mouse-only domain types. No I/O here — portable to Rust as-is."""

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


class CoordSpace(str, Enum):
    """Coordinate space tag for scale_coordinates (no global state)."""

    DESKTOP = "desktop"
    GAME = "game"


def scale_coordinates(
    x: int, y: int, *, game_dpi: float, desktop_dpi: float, sens: float
) -> tuple[int, int]:
    """Scale desktop pixels to game counts: round(v * game_dpi / desktop_dpi * sens).

    Pure helper; all factors must be > 0.
    """
    if game_dpi <= 0:
        raise ValueError(f"game_dpi out of range: {game_dpi}")
    if desktop_dpi <= 0:
        raise ValueError(f"desktop_dpi out of range: {desktop_dpi}")
    if sens <= 0:
        raise ValueError(f"sens out of range: {sens}")
    factor = game_dpi / desktop_dpi * sens
    return round(x * factor), round(y * factor)
