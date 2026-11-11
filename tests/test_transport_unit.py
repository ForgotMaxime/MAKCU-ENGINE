"""MakcuTransport unit tests with a mocked serial module (no hardware)."""

from __future__ import annotations

import time
from collections import deque
from unittest import mock

import pytest

from makcu_engine import errors as errs
from makcu_engine import transport as tmod


class FakeSerialException(Exception):
    """Stand-in for serial.SerialException."""


class FakeSerial:
    """In-memory serial port: canned read_until queue + auto query answers."""

    def __init__(self, port, baudrate=4000000, timeout=0.8, **kwargs):  # noqa: ANN001, ANN002, ANN003
        self.port = port
        self.baudrate = baudrate
        self.timeout = timeout
        self.write_timeout = kwargs.get("write_timeout", timeout)
        self.written: list[bytes] = []
        self._reads: deque[bytes] = deque()
        self.seen_timeouts: list[float] = []
        self.closed = False

    def reset_input_buffer(self) -> None:
        return None

    def reset_output_buffer(self) -> None:
        return None

    def write(self, data: bytes) -> int:
        self.written.append(bytes(data))
        return len(data)

    def flush(self) -> None:
        return None

    def close(self) -> None:
        self.closed = True

    def read_until(self, expected: bytes = b"") -> bytes:
        self.seen_timeouts.append(self.timeout)
        if self._reads:
            return self._reads.popleft()
        if self.written:
            last = self.written[-1]
            if last.startswith(b"km.version"):
                return b"km.MAKCU\r\n>>> "
            if last.startswith(b"km.echo"):
                return b"0\r\n>>> "
        return b""

    def read(self, n: int = 1) -> bytes:
        if self._reads:
            chunk = self._reads.popleft()
            return bytes(chunk[:n])
        return b""


def _make_transport(**kwargs):
    kwargs.setdefault("timeout_s", 0.2)
    return tmod.MakcuTransport("COM_FAKE", 4_000_000, **kwargs)


def _connect(t):
    with (
        mock.patch.object(tmod.serial, "Serial", FakeSerial),
        mock.patch.object(tmod.serial, "SerialException", FakeSerialException),
    ):
        t.connect()
    return t


def test_query_strips_prompt_and_echo():
    t = _make_transport()
    _connect(t)
    try:
        # Plain reply.
        assert t._ser is not None
        t._ser._reads.append(b"km.MAKCU\r\n>>> ")  # type: ignore[attr-defined]
        assert t.query("km.version") == "km.MAKCU"
        # Echo-on reply: last line wins.
        t._ser._reads.append(b"km.echo(1)\r\n0\r\n>>> ")  # type: ignore[attr-defined]
        assert t.query("km.echo", 1) == "0"
    finally:
        t.close()


def test_setter_err_window_reports_command_name():
    t = _make_transport()
    _connect(t)
    try:
        assert t._ser is not None
        t._ser._reads.append(b"ERR bad\r\n>>> ")  # type: ignore[attr-defined]
        with pytest.raises(errs.MakcuProtocolError, match="km.move"):
            t.set("km.move", 1, 0)
        # Success path: empty ERR window means silent setter ok.
        t._ser._reads.append(b"")  # type: ignore[attr-defined]
        t.set("km.move", 1, 0)
    finally:
        t.close()


def test_setter_error_window_zero_fast_path():
    t = _make_transport()
    _connect(t)
    try:
        assert t._ser is not None
        prev = t._ser.timeout  # type: ignore[attr-defined]
        t._ser._reads.append(b"")  # type: ignore[attr-defined]
        started = time.monotonic()
        t.set("km.move", 2, 0, error_window_s=0)
        assert time.monotonic() - started < 0.5
        # Timeout seen as 0 during the ERR read, then restored.
        assert t._ser.seen_timeouts[-1] == 0  # type: ignore[attr-defined]
        assert t._ser.timeout == prev  # type: ignore[attr-defined]
    finally:
        t.close()


def test_no_wait_degrades_set_to_fire_and_forget():
    t = _make_transport()
    _connect(t)
    try:
        assert t._ser is not None
        # Even with an ERR frame queued, no_wait must not raise.
        t._ser._reads.append(b"ERR boom\r\n>>> ")  # type: ignore[attr-defined]
        with t.no_wait():
            t.set("km.move", 1, 0)
        assert t._ser._reads  # ERR frame untouched by fire-and-forget  # type: ignore[attr-defined]
        # Outside no_wait the same ERR surfaces with the command name.
        with pytest.raises(errs.MakcuProtocolError, match="km.move"):
            t.set("km.move", 1, 0)
    finally:
        t.close()


def test_stats_ring_profiler_contents():
    t = _make_transport()
    _connect(t)
    try:
        assert t._ser is not None
        t.enable_profiling()
        t._ser._reads.append(b"km.MAKCU\r\n>>> ")  # type: ignore[attr-defined]
        assert t.query("km.version") == "km.MAKCU"
        t._ser._reads.append(b"")  # type: ignore[attr-defined]
        t.set("km.move", 1, 0)
        prof = t.get_profile()
        assert set(prof) == {"km.version", "km.move"}
        assert prof["km.version"]["count"] == 1
        assert prof["km.move"]["count"] == 1
        stats = t.get_stats()
        assert stats["cmds_sent"] == 2
        assert stats["tx_bytes"] == len(b"km.version()\rkm.move(1,0)\r")
        assert stats["avg_rtt_ms"] >= 0.0
        lines = t.get_recent_lines()
        assert lines[0] == "TX km.version()\r"
        assert "km.MAKCU" in lines[1]
        assert any(line.startswith("TX km.move(1,0)") for line in lines)
    finally:
        t.close()


def test_reconnect_counts_and_reopens():
    t = _make_transport()
    _connect(t)
    try:
        assert t.get_stats()["reconnects"] == 0
        with (
            mock.patch.object(tmod.serial, "Serial", FakeSerial),
            mock.patch.object(tmod.serial, "SerialException", FakeSerialException),
            mock.patch.object(tmod.time, "sleep", return_value=None),
        ):
            t.reconnect()
        assert t.get_stats()["reconnects"] == 1
        assert t.is_connected
        # Transport still usable after reconnect.
        assert t._ser is not None
        t._ser._reads.append(b"km.MAKCU\r\n>>> ")  # type: ignore[attr-defined]
        assert t.query("km.version") == "km.MAKCU"
    finally:
        with mock.patch.object(tmod.time, "sleep", return_value=None):
            t.close()


def test_connect_failure_raises_connection_error():
    t = _make_transport()

    def _boom(*args, **kwargs):  # noqa: ANN001, ANN002
        raise FakeSerialException("no port")

    with (
        mock.patch.object(tmod.serial, "Serial", _boom),
        mock.patch.object(tmod.serial, "SerialException", FakeSerialException),
        mock.patch.object(tmod.time, "sleep", return_value=None),
    ):
        with pytest.raises(errs.MakcuConnectionError):
            t.connect(retries=2)


def test_query_truncated_reply_raises_protocol_error():
    t = _make_transport()
    _connect(t)
    try:
        assert t._ser is not None
        t._ser._reads.append(b"km.MAKCU\r\n")  # type: ignore[attr-defined]
        with pytest.raises(errs.MakcuProtocolError):
            t.query("km.version", retries=0)
    finally:
        t.close()
