"""Transport interface. Mouse depends on this, never on serial directly."""

from __future__ import annotations

import contextlib
from collections.abc import Callable, Iterator
from typing import Protocol


class Transport(Protocol):
    """Serial-or-mock I/O used by Mouse."""

    @property
    def is_connected(self) -> bool:
        """Whether the port is currently open."""
        ...

    def query(self, name: str, *args: int | str, retries: int = 1) -> str:
        """Blocking query; returns the prompt-stripped payload."""
        ...

    def set(self, name: str, *args: int | str, error_window_s: float = 0.08) -> None:
        """Silent setter; errors surface through the ERR window."""
        ...

    def fire_and_forget(self, name: str, *args: int | str) -> None:
        """Write without waiting for anything back."""
        ...

    def send_raw(self, data: bytes, error_window_s: float = 0.08) -> None:
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

    def connect(self, retries: int = 10) -> None:
        """Open the port (real: exclusive serial with backoff retry)."""
        ...

    def close(self) -> None:
        """Close the port; redundant close is silent."""
        ...

    def reconnect(self) -> None:
        """Close then reopen the port."""
        ...

    def ensure_connected(self) -> None:
        """Ping and reconnect once if the link is down."""
        ...

    def on_connection_change(self, cb: Callable[[bool], None]) -> Callable[[bool], None]:
        """Register a callback run with True on connect, False on disconnect."""
        ...

    def remove_connection_callback(self, cb: Callable[[bool], None]) -> None:
        """Unregister a connection callback. Missing entries are ignored."""
        ...

    def get_stats(self) -> dict[str, int | float | str | None]:
        """Snapshot of I/O counters: tx_bytes, cmds_sent, reconnects, avg_rtt_ms, last_error."""
        ...

    def reset_stats(self) -> None:
        """Zero all I/O counters and clear last_error (ring buffer kept)."""
        ...

    def get_recent_lines(self, n: int = 20) -> list[str]:
        """Last n raw TX/RX frames (oldest first, at most 200 kept)."""
        ...

    @contextlib.contextmanager
    def no_wait(self) -> Iterator[None]:
        """Inside the block, set() behaves as fire-and-forget (no ERR check)."""
        ...

    def enable_profiling(self) -> None:
        """Opt in to per-command timing; off by default."""
        ...

    def disable_profiling(self) -> None:
        """Opt out of per-command timing; recorded data is discarded."""
        ...

    def profiling_enabled(self) -> bool:
        """Whether per-command timing is currently recorded."""
        ...

    def get_profile(self) -> dict[str, dict[str, float | int]]:
        """Snapshot of per-command timings: count/total_s/avg_ms/min_ms/max_ms."""
        ...

    def reset_profile(self) -> None:
        """Clear per-command timings (keeps profiling enabled state)."""
        ...
