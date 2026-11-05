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
from .mouse import Mouse
from .transport import MakcuTransport, list_candidates
from .types import Button, ButtonOwnership, DeviceInfo, LockTarget, Position, Screen

__version__ = "0.5.0"

__all__ = [
    "BAUDS",
    "PROMPT",
    "TIMEOUT_S",
    "USB_PID",
    "USB_VID",
    "BatchBuilder",
    "Button",
    "ButtonMonitor",
    "ButtonOwnership",
    "DeviceInfo",
    "LockTarget",
    "MakcuConnectionError",
    "MakcuError",
    "MakcuProtocolError",
    "MakcuTimeoutError",
    "MakcuTransport",
    "MockTransport",
    "Mouse",
    "Position",
    "Screen",
    "Transport",
    "__version__",
    "list_candidates",
]
