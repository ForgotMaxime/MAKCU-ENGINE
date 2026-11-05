"""Mouse-only domain types. No I/O here — portable to Rust as-is."""

from dataclasses import dataclass
from enum import Enum, IntEnum


class Button(IntEnum):
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
    NONE = 0
    PHYSICAL = 1
    INJECTED = 2
    BOTH = 3


@dataclass(frozen=True)
class DeviceInfo:
    route: str
    mouse_period: str
    keyboard_period: str
    controller_period: str


@dataclass(frozen=True)
class Screen:
    width: int
    height: int


@dataclass(frozen=True)
class Position:
    x: int
    y: int


class LockTarget(str, Enum):
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
