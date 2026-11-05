"""Typed errors. Never shadow builtins — all prefixed with Makcu."""


class MakcuError(Exception):
    pass


class MakcuConnectionError(MakcuError):
    pass


class MakcuTimeoutError(MakcuError):
    pass


class MakcuProtocolError(MakcuError, ValueError):
    """Protocol/framing/validation failure. Also a ValueError for stdlib compat."""
