"""Event loop, discovery helpers, monitor — mock-level plus hardware coexistence."""

import time

import pytest

serial = pytest.importorskip("serial")

from makcu_engine import MakcuTransport, Mouse
from makcu_engine.errors import (
    MakcuConnectionError,
    MakcuError,
    MakcuProtocolError,
    MakcuTimeoutError,
)
from makcu_engine.events import ButtonMonitor
from makcu_engine.mock import MockTransport


def test_detect_all_empty():
    assert MakcuTransport.detect_all(candidates=["COM_DOES_NOT_EXIST"]) == []


def test_wait_for_device_times_out():
    try:
        MakcuTransport.wait_for_device(timeout_s=0.2, poll_s=0.05,
                                       candidates=["COM_DOES_NOT_EXIST"])
        raise AssertionError("should raise")
    except MakcuTimeoutError:
        pass


def test_monitor_pump_dispatch():
    t = MockTransport()
    m = Mouse(t)
    mon = ButtonMonitor(m)
    seen: list[int] = []
    mon.on_change(seen.append)
    m.start_button_stream(1)
    t.push_stream_frame(b"km.\x01\r\n")
    assert mon.pump_once(0.1) == 0x01
    assert mon.pump_once(0.05) is None
    assert mon.drain() == [0x01]
    t.push_stream_frame(b"km.\x01\r\n")
    assert mon.pump_once(0.1) == 0x01
    assert mon.drain() == []
    m.stop_button_stream()


def test_hardware_loop_queries_during_stream():
    try:
        port, baud = MakcuTransport.detect()
    except MakcuError as e:
        pytest.skip(f"no MAKCU detected: {e}")
    with MakcuTransport(port, baud) as t:
        m = Mouse(t)
        t.start_event_loop()
        try:
            m.start_button_stream(1)
            assert "MAKCU" in m.version()
            assert m.device().route != ""
            assert m.moving() is False
            m.move(2, 0)
            m.move(-2, 0)
            assert m.wait_idle(timeout_s=2.0) is True
        finally:
            try:
                m.stop_button_stream()
            except MakcuProtocolError:
                pass
            t.stop_event_loop()


def test_event_loop_reader_death_raises_connection():
    """A dead reader must surface MakcuConnectionError, not timeouts."""

    class DeadSerial:
        timeout = 0.8

        def read(self, n):
            raise serial.SerialException("gone")

        def write(self, data):
            return len(data)

        def flush(self):
            pass

        def reset_input_buffer(self):
            pass

        def close(self):
            pass

    t = MakcuTransport("COMX", 4_000_000)
    t._ser = DeadSerial()  # type: ignore[assignment]
    t.start_event_loop()
    try:
        for _ in range(200):
            if not t._loop._thread.is_alive():  # type: ignore[union-attr]
                break
            time.sleep(0.01)
        assert not t._loop._thread.is_alive()  # type: ignore[union-attr]
        try:
            t.query("km.version")
            raise AssertionError("should raise")
        except MakcuConnectionError:
            pass
        try:
            t.poll_event(0.1)
            raise AssertionError("should raise")
        except MakcuConnectionError:
            pass
    finally:
        t.stop_event_loop()
        t.close()
