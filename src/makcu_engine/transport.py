"""Exclusive serial transport. Single reader/writer, thread-safe.

Port and baud are never hardcoded in the flow: use :meth:`detect` to pick
``(port, baud)`` at runtime, then open explicitly. :meth:`auto` is the
one-call shortcut (detect + connect).

Concurrency: by default one operation owns the port at a time (queries reset
the input buffer). :meth:`start_event_loop` switches to full-duplex mode: a
background reader demultiplexes prompt replies vs stream frames, so queries
keep working while a button stream is active. Framed modes 1/2 only — bare
mode 3 stays on the legacy poll path.
"""

from __future__ import annotations

import contextlib
import logging
import queue
import threading
import time
from collections import deque
from collections.abc import Iterator

import serial
import serial.tools.list_ports

from . import config
from .errors import MakcuConnectionError, MakcuError, MakcuProtocolError, MakcuTimeoutError
from .protocol import build_cmd, strip_prompt

logger = logging.getLogger(__name__)


def list_candidates() -> list[str]:
    """Order ports: CH343 VID/PID match first, other USB serial, then rest."""
    try:
        infos = list(serial.tools.list_ports.comports())
    except OSError:
        return []
    scored: list[tuple[int, str]] = []
    for info in infos:
        name = info.device
        desc = (info.description or "").upper()
        if info.vid == config.USB_VID and info.pid == config.USB_PID:
            score = 0
        elif info.vid is not None:
            score = 1
        elif "USB" in desc or "SERIAL" in desc:
            score = 2
        else:
            score = 3
        scored.append((score, name))
    scored.sort()
    return [name for _, name in scored]


class _EventLoop:
    """Background reader: prompt replies -> replies queue, stream lines -> events queue."""

    def __init__(self, transport: MakcuTransport) -> None:
        self._t = transport
        self._stop = threading.Event()
        self._replies: queue.Queue[bytes] = queue.Queue()
        self._events: queue.Queue[bytes] = queue.Queue()
        self._thread = threading.Thread(target=self._run, daemon=True, name="makcu-events")

    def start(self) -> None:
        self._thread.start()

    def stop(self, timeout_s: float = 1.0) -> None:
        self._stop.set()
        self._thread.join(timeout_s)

    def _run(self) -> None:
        from .protocol import split_frames

        t = self._t
        with t._lock:
            if t._ser is None:
                return
            prev = t._ser.timeout
            t._ser.timeout = 0.01
        try:
            buf = bytearray()
            while not self._stop.is_set():
                with t._lock:
                    if t._ser is None:
                        break
                    try:
                        chunk = t._ser.read(256)
                    except serial.SerialException:
                        break
                    if chunk:
                        buf += chunk
                if len(buf) > 65536:
                    del buf[:-65536]
                if chunk or buf:
                    replies, events, rest = split_frames(bytes(buf))
                    buf = bytearray(rest)
                    for r in replies:
                        self._replies.put(r)
                    for e in events:
                        self._events.put(e)
                if not chunk:
                    time.sleep(0.005)
        finally:
            with t._lock:
                if t._ser is not None:
                    t._ser.timeout = prev

    def request(self, payload: bytes, timeout_s: float) -> bytes:
        # No reset_input_buffer here: the reader owns the input stream.
        # Empty return = silent setter (echo off), like the legacy path.
        t = self._t
        if not self._thread.is_alive():
            raise MakcuConnectionError("event reader died")
        while True:
            try:
                self._replies.get_nowait()
            except queue.Empty:
                break
        with t._lock:
            if t._ser is None:
                raise MakcuConnectionError("transport not connected")
            try:
                t._ser.write(payload)
                t._ser.flush()
            except serial.SerialException as e:
                raise MakcuConnectionError(f"write failed: {e}") from e
        try:
            return self._replies.get(timeout=timeout_s)
        except queue.Empty:
            if not self._thread.is_alive():
                raise MakcuConnectionError("event reader died") from None
            return b""


class MakcuTransport:
    """Exclusive serial transport with optional demux event loop."""
    def __init__(self, port: str, baudrate: int, timeout_s: float = config.TIMEOUT_S) -> None:
        self._port = port
        self._baudrate = baudrate
        self._timeout_s = timeout_s
        self._lock = threading.Lock()
        self._req = threading.Lock()
        self._ser: serial.Serial | None = None
        self._loop: _EventLoop | None = None
        # Observability (counting only; never changes I/O behavior).
        self._obs = threading.Lock()
        self._tx_bytes = 0
        self._cmds_sent = 0
        self._reconnects = 0
        self._rtt_total_s = 0.0
        self._rtt_count = 0
        self._last_error: str | None = None
        self._recent: deque[str] = deque(maxlen=200)
        self._no_wait_depth = 0

    @property
    def port(self) -> str:
        """Port this transport was opened on."""
        return self._port

    @property
    def baudrate(self) -> int:
        """Baud rate this transport runs at."""
        return self._baudrate

    @property
    def is_connected(self) -> bool:
        """Whether the serial port is currently open."""
        return self._ser is not None

    def connect(self, retries: int = 10) -> None:
        """Open the port exclusively (8N1, no flow control) with backoff retry."""
        last: Exception | None = None
        for attempt in range(retries):
            try:
                self._ser = serial.Serial(
                    self._port,
                    baudrate=self._baudrate,
                    bytesize=8,
                    parity="N",
                    stopbits=1,
                    timeout=self._timeout_s,
                    write_timeout=self._timeout_s,
                    xonxoff=False,
                    rtscts=False,
                    dsrdtr=False,
                    exclusive=True,
                )
                self._ser.reset_input_buffer()
                self._ser.reset_output_buffer()
                return
            except serial.SerialException as e:
                last = e
                time.sleep(0.2 * (attempt + 1))
        raise MakcuConnectionError(f"cannot open {self._port}@{self._baudrate}: {last}") from last

    def close(self) -> None:
        """Stop the event loop if running, then close the port."""
        self.stop_event_loop()
        if self._ser is not None:
            try:
                self._ser.close()
            finally:
                self._ser = None

    def __enter__(self) -> MakcuTransport:  # noqa: PYI034
        if self._ser is None:
            self.connect()
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()

    @classmethod
    def probe(cls, port: str, timeout_s: float = config.TIMEOUT_S) -> int:
        """Baud-scan one port with km.version(); returns the working baud."""
        for baud in config.BAUDS:
            ser = None
            try:
                ser = serial.Serial(port, baudrate=baud, timeout=timeout_s,
                                    write_timeout=timeout_s, exclusive=True)
                ser.reset_input_buffer()
                ser.write(build_cmd("km.version"))
                ser.flush()
                raw = ser.read_until(config.PROMPT)
                if b"km.MAKCU" in raw:
                    return baud
            except serial.SerialException:
                continue
            finally:
                if ser is not None:
                    try:
                        ser.close()
                    finally:
                        time.sleep(0.15)
        raise MakcuConnectionError(f"no MAKCU on {port}")

    @classmethod
    def detect(
        cls,
        timeout_s: float = config.TIMEOUT_S,
        candidates: list[str] | None = None,
    ) -> tuple[str, int]:
        """First candidate that answers km.MAKCU, with its baud."""
        for port in candidates if candidates is not None else list_candidates():
            try:
                return port, cls.probe(port, timeout_s)
            except MakcuConnectionError:
                continue
        raise MakcuConnectionError("no MAKCU detected on any serial port")

    @classmethod
    def detect_all(
        cls,
        timeout_s: float = config.TIMEOUT_S,
        candidates: list[str] | None = None,
    ) -> list[tuple[str, int]]:
        """Every candidate that answers, for multi-device setups."""
        found: list[tuple[str, int]] = []
        for port in candidates if candidates is not None else list_candidates():
            try:
                found.append((port, cls.probe(port, timeout_s)))
            except MakcuConnectionError:
                continue
        return found

    @classmethod
    def wait_for_device(
        cls,
        timeout_s: float = 30.0,
        poll_s: float = 1.0,
        candidates: list[str] | None = None,
    ) -> tuple[str, int]:
        """Block until a MAKCU appears (hot-plug). Raises MakcuTimeoutError."""
        deadline = time.monotonic() + timeout_s
        while True:
            try:
                return cls.detect(config.TIMEOUT_S, candidates)
            except MakcuConnectionError:
                if time.monotonic() >= deadline:
                    raise MakcuTimeoutError("no MAKCU appeared in time") from None
                time.sleep(min(poll_s, max(0.0, deadline - time.monotonic())))

    @classmethod
    def auto(cls, timeout_s: float = config.TIMEOUT_S) -> MakcuTransport:
        """Detect a MAKCU, then open it. One-call shortcut."""
        port, baud = cls.detect(timeout_s)
        time.sleep(0.3)
        t = cls(port, baud, timeout_s)
        t.connect()
        return t

    def start_event_loop(self) -> None:
        """Start the background demux reader (framed modes 1/2).

        While running, queries and stream polls share the port safely:
        replies go to the pending request, stream lines to the event queue.
        """
        if self._ser is None:
            raise MakcuConnectionError("transport not connected")
        if self._loop is not None:
            return
        with self._lock:
            self._ser.reset_input_buffer()
        loop = _EventLoop(self)
        self._loop = loop
        try:
            loop.start()
        except Exception:
            self._loop = None
            raise

    def stop_event_loop(self, timeout_s: float = 1.0) -> None:
        """Stop the background demux reader."""
        loop, self._loop = self._loop, None
        if loop is not None:
            loop.stop(timeout_s)

    def event_loop_running(self) -> bool:
        """Whether the demux reader is active."""
        return self._loop is not None

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
        """Inside the block, set() behaves as fire-and-forget (no ERR window read).

        Queries still block for their replies; send_raw() is unaffected
        (Batch keeps its single ERR-window read). May nest.
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

    def _note_rx(self, raw: bytes) -> None:
        """Append one received frame to the ring buffer."""
        with self._obs:
            try:
                self._recent.append(f"RX {raw.decode('ascii')}")
            except UnicodeDecodeError:
                self._recent.append(f"RX {raw!r}")

    def _note_rtt(self, dt_s: float) -> None:
        """Fold one query round-trip into the running average."""
        with self._obs:
            self._rtt_total_s += dt_s
            self._rtt_count += 1

    def _note_error(self, e: Exception) -> None:
        """Remember the last transport/protocol error message."""
        with self._obs:
            self._last_error = str(e)

    def _note_reconnect(self) -> None:
        """Count one reconnect."""
        with self._obs:
            self._reconnects += 1

    def poll_event(self, timeout_s: float = 0.5) -> bytes:
        """Next queued stream frame; fails fast if the reader died."""
        loop = self._loop
        if loop is None:
            raise MakcuProtocolError("event loop not running")
        if not loop._thread.is_alive():
            raise MakcuConnectionError("event reader died")
        try:
            return loop._events.get(timeout=timeout_s)
        except queue.Empty:
            raise MakcuTimeoutError("no event") from None

    def _exchange(self, payload: bytes) -> bytes:
        raw: bytes
        self._note_tx(payload)
        if self._loop is not None:
            with self._req:
                raw = self._loop.request(payload, self._timeout_s)
        else:
            if self._ser is None:
                err = MakcuConnectionError("transport not connected")
                self._note_error(err)
                raise err
            logger.debug("TX %r", payload)
            with self._lock:
                self._ser.reset_input_buffer()
                self._ser.write(payload)
                self._ser.flush()
                raw = self._ser.read_until(config.PROMPT)
        logger.debug("RX %r", raw)
        self._note_rx(raw)
        if not raw:
            raise MakcuTimeoutError("no reply (prompt >>> missing)")
        if config.PROMPT not in raw:
            raise MakcuProtocolError(f"truncated reply: {raw!r}")
        return raw

    def query(self, name: str, *args: int | str, retries: int = 1) -> str:
        """Blocking query with one retry on timeout; returns stripped payload."""
        last: MakcuTimeoutError = MakcuTimeoutError("no reply")
        start = time.monotonic()
        try:
            for _ in range(max(retries, 0) + 1):
                try:
                    raw = self._exchange(build_cmd(name, *args))
                    break
                except MakcuTimeoutError as e:
                    last = e
                    logger.debug("query %s timeout, retrying", name)
                    continue
            else:
                raise last
        except MakcuError as e:
            self._note_error(e)
            raise
        self._note_rtt(time.monotonic() - start)
        return strip_prompt(raw)

    def set(self, name: str, *args: int | str, error_window_s: float = 0.08) -> None:
        """Silent setter; the command name is attached to protocol errors.

        Inside no_wait(), degrades to fire_and_forget (no ERR window read).
        """
        if self._no_wait_active():
            self.fire_and_forget(name, *args)
            return
        try:
            self.send_raw(build_cmd(name, *args), error_window_s)
        except MakcuProtocolError as e:
            wrapped = MakcuProtocolError(f"{name} failed: {e}")
            self._note_error(wrapped)
            raise wrapped from e

    def send_raw(self, data: bytes, error_window_s: float = 0.08) -> None:
        """Write pre-framed bytes; single ERR-window read. Used by Batch."""
        logger.debug("TX %r", data)
        self._note_tx(data)
        if self._loop is not None:
            with self._req:
                raw = self._loop.request(data, error_window_s)
        else:
            if self._ser is None:
                err = MakcuConnectionError("transport not connected")
                self._note_error(err)
                raise err
            with self._lock:
                self._ser.reset_input_buffer()
                self._ser.write(data)
                self._ser.flush()
                prev = self._ser.timeout
                try:
                    self._ser.timeout = error_window_s
                    raw = self._ser.read_until(config.PROMPT)
                finally:
                    self._ser.timeout = prev
        logger.debug("RX %r", raw)
        self._note_rx(raw)
        if b"ERR" in raw:
            dev_err = MakcuProtocolError(f"device ERR: {raw!r}")
            self._note_error(dev_err)
            raise dev_err

    def read_line(self, timeout_s: float = 0.5) -> bytes:
        """Read one CRLF-terminated frame without touching the buffer.

        For stream polling only — never call while a query is in flight.
        """
        if self._ser is None:
            raise MakcuConnectionError("transport not connected")
        with self._lock:
            prev = self._ser.timeout
            try:
                self._ser.timeout = timeout_s
                raw: bytes = self._ser.read_until(b"\r\n")
            finally:
                self._ser.timeout = prev
        logger.debug("STREAM %r", raw)
        if not raw:
            raise MakcuTimeoutError("no stream frame")
        return raw

    def read_exact(self, n: int, timeout_s: float = 0.5) -> bytes:
        """Read exactly n bytes without touching the buffer (mode-3 streams)."""
        if self._ser is None:
            raise MakcuConnectionError("transport not connected")
        if n <= 0:
            raise MakcuProtocolError(f"read_exact n={n}")
        with self._lock:
            prev = self._ser.timeout
            try:
                self._ser.timeout = timeout_s
                raw: bytes = self._ser.read(n)
            finally:
                self._ser.timeout = prev
        logger.debug("STREAM %r", raw)
        if len(raw) < n:
            raise MakcuTimeoutError("short stream read")
        return raw

    def reconnect(self) -> None:
        """Close, cool down, and reopen the port."""
        self._note_reconnect()
        self.close()
        time.sleep(0.3)
        self.connect()

    def ensure_connected(self) -> None:
        """Ping and reconnect once if the link is down."""
        try:
            self.query("km.echo")
            return
        except MakcuConnectionError:
            pass
        self.reconnect()
        self.query("km.echo")

    def fire_and_forget(self, name: str, *args: int | str) -> None:
        """Write without waiting for anything back."""
        if self._ser is None:
            err = MakcuConnectionError("transport not connected")
            self._note_error(err)
            raise err
        payload = build_cmd(name, *args)
        with self._lock:
            self._ser.reset_output_buffer()
            self._ser.write(payload)
            self._ser.flush()
        self._note_tx(payload)
