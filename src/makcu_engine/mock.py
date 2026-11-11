"""In-memory transport. Same interface as serial, for tests and offline dev."""

from __future__ import annotations

import contextlib
import logging
import threading
import time
from collections import deque
from collections.abc import Callable, Iterator

from .errors import MakcuConnectionError, MakcuProtocolError, MakcuTimeoutError
from .protocol import build_cmd

logger = logging.getLogger(__name__)


class MockTransport:
    """In-memory Transport for tests and offline development."""

    def __init__(self, replies: dict[str, str] | None = None) -> None:
        self.replies = replies or {}
        self.sent: list[str] = []
        self._stream = bytearray()
        self.fail_next_send = False
        self._loop_active = False
        self._connected = False
        self._conn_cbs: list[Callable[[bool], None]] = []
        self._conn_cbs_lock = threading.Lock()
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
        self._profile: dict[str, list[float]] | None = None

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

    def enable_profiling(self) -> None:
        """Opt in to per-command timing; off by default."""
        with self._obs:
            if self._profile is None:
                self._profile = {}

    def disable_profiling(self) -> None:
        """Opt out of per-command timing; recorded data is discarded."""
        with self._obs:
            self._profile = None

    def profiling_enabled(self) -> bool:
        """Whether per-command timing is currently recorded."""
        return self._profile is not None

    def get_profile(self) -> dict[str, dict[str, float | int]]:
        """Snapshot of per-command timings: count/total_s/avg_ms/min_ms/max_ms."""
        with self._obs:
            prof = dict(self._profile) if self._profile is not None else {}
            acc: dict[str, list[float]] = {k: list(v) for k, v in prof.items()}
        out: dict[str, dict[str, float | int]] = {}
        for cmd, ent in acc.items():
            count = int(ent[0])
            total = ent[1]
            out[cmd] = {
                "count": count,
                "total_s": total,
                "avg_ms": (total / count * 1000.0) if count else 0.0,
                "min_ms": ent[2] * 1000.0,
                "max_ms": ent[3] * 1000.0,
            }
        return out

    def reset_profile(self) -> None:
        """Clear per-command timings (keeps profiling enabled state)."""
        with self._obs:
            if self._profile is not None:
                self._profile.clear()

    def _record_profile(self, cmd: str, dt_s: float) -> None:
        """Fold one query/set duration into the per-command table."""
        prof = self._profile
        if prof is None:
            return
        with self._obs:
            ent = prof.get(cmd)
            if ent is None:
                prof[cmd] = [1.0, dt_s, dt_s, dt_s]
            else:
                ent[0] += 1.0
                ent[1] += dt_s
                if dt_s < ent[2]:
                    ent[2] = dt_s
                if dt_s > ent[3]:
                    ent[3] = dt_s

    def get_recent_lines(self, n: int = 20) -> list[str]:
        """Last n raw TX/RX frames (oldest first, at most 200 kept)."""
        if n <= 0:
            return []
        with self._obs:
            return list(self._recent)[-n:]

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
            try:
                self._recent.append(f"TX {payload.decode('ascii')}")
            except UnicodeDecodeError:
                self._recent.append(f"TX {payload!r}")

    def _note_rx(self, reply: str) -> None:
        """Append one canned reply to the ring buffer."""
        with self._obs:
            self._recent.append(f"RX {reply}")

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
        if base.split(".")[-1] in ("left", "right", "middle", "side1", "side2", "ms1", "ms2"):
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

    def query(self, name: str, *args: int | str, retries: int = 1) -> str:
        """Canned reply lookup; records the command (retries ignored)."""
        prof_on = self._profile is not None
        t_prof = time.perf_counter() if prof_on else 0.0
        try:
            start = time.monotonic()
            self._note_tx(build_cmd(name, *args))
            reply = self._lookup(name, args)
            with self._obs:
                self._rtt_total_s += time.monotonic() - start
                self._rtt_count += 1
            self._note_rx(reply)
            return reply
        finally:
            if prof_on:
                self._record_profile(name, time.perf_counter() - t_prof)

    def set(self, name: str, *args: int | str, error_window_s: float = 0.08) -> None:
        """Record a setter; honors the fail-next flag (skipped inside no_wait)."""
        if self._no_wait_active():
            self.fire_and_forget(name, *args)
            return
        prof_on = self._profile is not None
        t_prof = time.perf_counter() if prof_on else 0.0
        try:
            if self.fail_next_send:
                self.fail_next_send = False
                err = MakcuProtocolError(f"{name} failed: device ERR: {name!r}")
                with self._obs:
                    self._last_error = str(err)
                raise err
            self._note_tx(build_cmd(name, *args))
            self._lookup(name, args)
        finally:
            if prof_on:
                self._record_profile(name, time.perf_counter() - t_prof)

    def fire_and_forget(self, name: str, *args: int | str) -> None:
        """Record without reply."""
        self._note_tx(build_cmd(name, *args))
        self.sent.append(f"{name}({','.join(str(a) for a in args)})")

    def send_raw(self, data: bytes, error_window_s: float = 0.08) -> None:
        """Record raw bytes; honors the fail-next flag."""
        if self.fail_next_send:
            self.fail_next_send = False
            err = MakcuProtocolError(f"device ERR: {data[:32]!r}")
            with self._obs:
                self._last_error = str(err)
            raise err
        self._note_tx(data)
        try:
            self.sent.append(data.decode("ascii"))
        except UnicodeDecodeError:
            self.sent.append(repr(data))

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
            raise MakcuProtocolError(f"n out of range: {n}")
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

    @property
    def is_connected(self) -> bool:
        """Whether the mock port is currently open."""
        return self._connected

    def on_connection_change(self, cb: Callable[[bool], None]) -> Callable[[bool], None]:
        """Register a callback run with True on connect, False on disconnect.

        Same snapshot-then-dispatch semantics as MakcuTransport: callbacks
        run synchronously on the connecting/closing thread from a snapshot,
        so registration during dispatch is safe, and a raising callback is
        logged and skipped without breaking the connect/close call.
        """
        with self._conn_cbs_lock:
            self._conn_cbs.append(cb)
        return cb

    def remove_connection_callback(self, cb: Callable[[bool], None]) -> None:
        """Unregister a connection callback. Missing entries are ignored."""
        with self._conn_cbs_lock:
            try:
                self._conn_cbs.remove(cb)
            except ValueError:
                pass

    def _notify_connection(self, connected: bool) -> None:
        """Dispatch a connection event to a snapshot of callbacks."""
        with self._conn_cbs_lock:
            cbs = list(self._conn_cbs)
        for cb in cbs:
            try:
                cb(connected)
            except Exception:
                logger.exception("connection callback failed")

    def connect(self, retries: int = 10) -> None:
        """Mark the mock port open and notify callbacks with True."""
        del retries
        self._connected = True
        self._notify_connection(True)

    def close(self) -> None:
        """Mark the mock port closed, notifying with False on transition."""
        was_connected = self._connected
        self._connected = False
        if was_connected:
            self._notify_connection(False)

    def reconnect(self) -> None:
        """Close then reopen the mock port (emits False then True)."""
        with self._obs:
            self._reconnects += 1
        self.close()
        self.connect()

    def ensure_connected(self) -> None:
        """Ping and reconnect once if the link is down (mock: no-op ping)."""
        try:
            self.query("km.echo")
            return
        except MakcuConnectionError:
            pass
        self.reconnect()
        self.query("km.echo")
