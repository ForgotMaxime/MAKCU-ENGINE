"""Transport interface. Mouse depends on this, never on serial directly."""

from __future__ import annotations

from typing import Protocol


class Transport(Protocol):
    """Serial-or-mock I/O used by Mouse."""

    def query(self, name: str, *args: int | str) -> str:
        """Blocking query; returns the prompt-stripped payload."""
        ...

    def set(self, name: str, *args: int | str) -> None:
        """Silent setter; errors surface through the ERR window."""
        ...

    def fire_and_forget(self, name: str, *args: int | str) -> None:
        """Write without waiting for anything back."""
        ...

    def send_raw(self, data: bytes) -> None:
        """Write pre-framed bytes; single ERR-window read."""
        ...

    def read_line(self, timeout_s: float = 0.5) -> bytes:
        """Read one CRLF-terminated frame without touching the buffer."""
        ...

    def read_exact(self, n: int, timeout_s: float = 0.5) -> bytes:
        """Read exactly n bytes without touching the buffer."""
        ...

    def start_event_loop(self) -> None:
        """Start the background demux reader (framed modes 1/2)."""
        ...

    def stop_event_loop(self, timeout_s: float = 1.0) -> None:
        """Stop the background demux reader."""
        ...

    def event_loop_running(self) -> bool:
        """Whether the demux reader is active."""
        ...

    def poll_event(self, timeout_s: float = 0.5) -> bytes:
        """Next queued stream frame; only with the event loop running."""
        ...
