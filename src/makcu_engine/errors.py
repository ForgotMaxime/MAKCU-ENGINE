"""Typed errors. Never shadow builtins — all prefixed with Makcu."""


class MakcuError(Exception):
    """Base error for every MAKCU failure."""

    pass


class MakcuConnectionError(MakcuError):
    """Serial port unavailable or I/O failure."""

    pass


class MakcuTimeoutError(MakcuError):
    """No reply within the deadline."""

    pass


class MakcuProtocolError(MakcuError, ValueError):
    """Protocol/framing/validation failure. Also a ValueError for stdlib compat."""
