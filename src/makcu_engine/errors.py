"""Typed errors. Never shadow builtins — all prefixed with Makcu."""


class MakcuError(Exception):
    pass


class MakcuConnectionError(MakcuError):
    pass


class MakcuTimeoutError(MakcuError):
    pass


class MakcuProtocolError(MakcuError):
    pass
