"""In-memory transport. Same interface as serial, for tests and offline dev."""

from __future__ import annotations

import contextlib
import threading
import time
from collections import deque
from collections.abc import Iterator

from .errors import MakcuProtocolError, MakcuTimeoutError
from .protocol import build_cmd


class MockTransport:
    """In-memory Transport for tests and offline development."""

    def __init__(self, replies: dict[str, str] | None = None) -> None:
        self.replies = replies or {}
        self.sent: list[str] = []
        self._stream = bytearray()
        self.fail_next_send = False
        self._loop_active = False
        # Observability mirror of MakcuTransport (counting only).
        self._obs = threading.Lock()
        self._tx_bytes = 0
        self._cmds_sent = 0
        self._reconnects = 0
        self._rtt_total_s = 0.0
        self._rtt_count = 0
        self._last_error: str | None = None
        self._recent: deque[str] = deque(maxlen=200)
        self._no_wait_depth = 0

    def get_stats(self) -> dict[str, int | float | str | None]:
        """Snapshot of I/O counters: tx_bytes, cmds_sent, reconnects, avg_rtt_ms, last_error."""
        with self._obs:
            avg = (self._rtt_total_s / self._rtt_count * 1000.0) if self._rtt_count else 0.0
            return {
                "tx_bytes": self._tx_bytes,
                "cmds_sent": self._cmds_sent,
                "reconnects": self._reconnects,
                "avg_rtt_ms": avg,
                "last_error": self._last_error,
            }

    def reset_stats(self) -> None:
        """Zero all I/O counters and clear last_error (ring buffer kept)."""
        with self._obs:
            self._tx_bytes = 0
            self._cmds_sent = 0
            self._reconnects = 0
            self._rtt_total_s = 0.0
            self._rtt_count = 0
            self._last_error = None

    def get_recent_lines(self, n: int = 20) -> list[str]:
        """Last n raw TX/RX frames (oldest first, at most 200 kept)."""
        with self._obs:
            return list(self._recent)[-max(n, 0):]

    @contextlib.contextmanager
    def no_wait(self) -> Iterator[None]:
        """Inside the block, set() behaves as fire-and-forget (no ERR check).

        Queries still return their canned replies; send_raw() is unaffected.
        May nest.
        """
        with self._obs:
            self._no_wait_depth += 1
        try:
            yield
        finally:
            with self._obs:
                self._no_wait_depth -= 1

    def _no_wait_active(self) -> bool:
        """Whether inside a no_wait() block."""
        with self._obs:
            return self._no_wait_depth > 0

    def _note_tx(self, payload: bytes) -> None:
        """Count one transmitted frame and append it to the ring buffer."""
        with self._obs:
            self._tx_bytes += len(payload)
            self._cmds_sent += 1
            self._recent.append(f"TX {payload.decode('ascii')}")

    def _note_rx(self, reply: str) -> None:
        """Append one canned reply to the ring buffer."""
        with self._obs:
            self._recent.append(f"RX {reply}")

    def reconnect(self) -> None:
        """Mock reconnect: counts only, link state unchanged."""
        with self._obs:
            self._reconnects += 1

    def push_stream_frame(self, frame: bytes) -> None:
        """Enqueue a raw stream frame for read_line / poll_event."""
        self._stream += frame

    def _lookup(self, name: str, args: tuple[int | str, ...]) -> str:
        cmd = f"{name}({','.join(str(a) for a in args)})"
        self.sent.append(cmd)
        base = cmd.split("(")[0]
        for key in (cmd, base, name):
            if key in self.replies:
                return self.replies[key]
        if base.startswith("km.lock_"):
            return "0"
        if base.split(".")[-1] in ("left", "right", "middle", "side1", "side2"):
            return "0"
        return {
            "km.version": "km.MAKCU",
            "km.device": "R:MK;M:16uf;K:8uf;C:0uf",
            "km.echo": "0",
            "km.moving": "0",
            "km.buttons": "0",
            "km.left": "0",
            "km.autorelease": "0",
            "km.api_version": "1",
            "km.getpos": "km.getpos(100,200)",
            "km.screen": "km.screen(1920,1080)",
        }.get(base, "")

    def query(self, name: str, *args: int | str) -> str:
        """Canned reply lookup; records the command."""
        start = time.monotonic()
        self._note_tx(build_cmd(name, *args))
        reply = self._lookup(name, args)
        with self._obs:
            self._rtt_total_s += time.monotonic() - start
            self._rtt_count += 1
        self._note_rx(reply)
        return reply

    def set(self, name: str, *args: int | str) -> None:
        """Record a setter; honors the fail-next flag (skipped inside no_wait)."""
        if self._no_wait_active():
            self.fire_and_forget(name, *args)
            return
        if self.fail_next_send:
            self.fail_next_send = False
            err = MakcuProtocolError(f"{name} failed: device ERR: {name!r}")
            with self._obs:
                self._last_error = str(err)
            raise err
        self._note_tx(build_cmd(name, *args))
        self._lookup(name, args)

    def fire_and_forget(self, name: str, *args: int | str) -> None:
        """Record without reply."""
        self._note_tx(build_cmd(name, *args))
        self.sent.append(f"{name}({','.join(str(a) for a in args)})")

    def send_raw(self, data: bytes) -> None:
        """Record raw bytes; honors the fail-next flag."""
        if self.fail_next_send:
            self.fail_next_send = False
            err = MakcuProtocolError(f"device ERR: {data[:32]!r}")
            with self._obs:
                self._last_error = str(err)
            raise err
        self._note_tx(data)
        self.sent.append(data.decode("ascii"))

    def read_line(self, timeout_s: float = 0.5) -> bytes:
        """Pop one CRLF-terminated frame from the queued stream bytes."""
        idx = self._stream.find(b"\r\n")
        if idx < 0:
            raise MakcuTimeoutError("no stream frame")
        line = bytes(self._stream[: idx + 2])
        del self._stream[: idx + 2]
        return line

    def read_exact(self, n: int, timeout_s: float = 0.5) -> bytes:
        """Pop exactly n bytes from the queued stream bytes."""
        if n <= 0:
            raise MakcuProtocolError(f"read_exact n={n}")
        if len(self._stream) < n:
            raise MakcuTimeoutError("short stream read")
        out = bytes(self._stream[:n])
        del self._stream[:n]
        return out

    def start_event_loop(self) -> None:
        """Mark the loop running (no thread; mock pops frames directly)."""
        self._loop_active = True

    def stop_event_loop(self, timeout_s: float = 1.0) -> None:
        """Mark the loop stopped."""
        self._loop_active = False

    def event_loop_running(self) -> bool:
        """Whether the mock loop flag is set."""
        return self._loop_active

    def poll_event(self, timeout_s: float = 0.5) -> bytes:
        """Pop the next queued stream frame."""
        return self.read_line(timeout_s)
