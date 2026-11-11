"""MAKCU V4 mouse-only engine — public surface."""

from .abc import Transport
from .batch import BatchBuilder
from .config import BAUDS, PROMPT, TIMEOUT_S, USB_PID, USB_VID
from .errors import (
    MakcuConnectionError,
    MakcuError,
    MakcuProtocolError,
    MakcuTimeoutError,
)
from .events import ButtonMonitor
from .mock import MockTransport
from .mouse import CLICK_PROFILES, Mouse
from .transport import MakcuTransport, list_candidates
from .types import (
    Bounds,
    Button,
    ButtonOwnership,
    CoordSpace,
    DeviceInfo,
    LockTarget,
    Monitor,
    Position,
    Screen,
)

__version__ = "0.5.1"

__all__ = [
    "BAUDS",
    "PROMPT",
    "TIMEOUT_S",
    "USB_PID",
    "USB_VID",
    "BatchBuilder",
    "Bounds",
    "Button",
    "ButtonMonitor",
    "ButtonOwnership",
    "CLICK_PROFILES",
    "CoordSpace",
    "DeviceInfo",
    "LockTarget",
    "MakcuConnectionError",
    "MakcuError",
    "MakcuProtocolError",
    "MakcuTimeoutError",
    "MakcuTransport",
    "MockTransport",
    "Monitor",
    "Mouse",
    "Position",
    "Screen",
    "Transport",
    "__version__",
    "list_candidates",
]
