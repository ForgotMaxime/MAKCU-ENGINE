"""Hardware integration — safe queries + net-zero moves. Skipped if no device."""

import pytest

serial = pytest.importorskip("serial")

from makcu_engine import MakcuTransport, Mouse
from makcu_engine.errors import MakcuError, MakcuProtocolError
from makcu_engine.types import Button, ButtonOwnership


def test_hardware_probes():
    try:
        port, baud = MakcuTransport.detect()
    except MakcuError as e:
        pytest.skip(f"no MAKCU detected: {e}")
    with MakcuTransport(port, baud) as t:
        m = Mouse(t)
        assert "MAKCU" in m.version()
        assert m.device().route != ""
        assert m.echo() in (0, 1)
        assert m.moving() is False
        for b in Button:
            assert isinstance(m.button_state(b), ButtonOwnership)
        m.move(2, 0)
        m.move(-2, 0)
        assert isinstance(m.getpos().x, int)


def test_hardware_batch_netzero():
    try:
        port, baud = MakcuTransport.detect()
    except MakcuError as e:
        pytest.skip(f"no MAKCU detected: {e}")
    with MakcuTransport(port, baud) as t:
        m = Mouse(t)
        n = m.batch().move(2, 0).move(-2, 0).wheel(0).execute()
        assert n == 3
        assert isinstance(m.getpos().x, int)


def test_hardware_testbuild_support():
    """Test-build features (V4.103+): assert only if firmware answers."""
    try:
        port, baud = MakcuTransport.detect()
    except MakcuError as e:
        pytest.skip(f"no MAKCU detected: {e}")
    with MakcuTransport(port, baud) as t:
        m = Mouse(t)
        try:
            before = m.autorelease()
        except MakcuProtocolError:
            pytest.skip("firmware without km.autorelease")
        m.set_autorelease(5000)
        assert m.autorelease() == 5000
        m.heartbeat()
        m.set_autorelease(before)
        assert m.autorelease() == before


def test_hardware_echo_on_queries():
    """Echo-aware parsing: same values with echo on and off."""
    try:
        port, baud = MakcuTransport.detect()
    except MakcuError as e:
        pytest.skip(f"no MAKCU detected: {e}")
    with MakcuTransport(port, baud) as t:
        m = Mouse(t)
        v0, route0 = m.version(), m.device().route
        m.set_echo(True)
        try:
            assert m.echo() == 1
            assert m.version() == v0
            assert m.device().route == route0
            assert m.getpos().x >= 0
            assert m.button_state(Button.LEFT) in list(ButtonOwnership)
        finally:
            m.set_echo(False)
        assert m.echo() == 0


def test_hardware_ms1_and_stream_state():
    try:
        port, baud = MakcuTransport.detect()
    except MakcuError as e:
        pytest.skip(f"no MAKCU detected: {e}")
    with MakcuTransport(port, baud) as t:
        m = Mouse(t)
        assert m.button_state(Button.MS1) == ButtonOwnership.NONE
        assert m.button_state(Button.SIDE1) == ButtonOwnership.NONE
        assert m.button_stream_state() is False
        m.start_button_stream(1)
        try:
            assert m.button_stream_state() is True
        finally:
            m.stop_button_stream()
        assert m.button_stream_state() is False


def test_hardware_smooth_netzero():
    try:
        port, baud = MakcuTransport.detect()
    except MakcuError as e:
        pytest.skip(f"no MAKCU detected: {e}")
    with MakcuTransport(port, baud) as t:
        m = Mouse(t)
        m.move_smooth(8, 4, steps=4, step_s=0)
        m.move(-8, -4)
        assert m.wait_idle(timeout_s=2.0) is True
        assert isinstance(m.getpos().x, int)


def test_hardware_wait_for_device_fast_path():
    try:
        port, baud = MakcuTransport.wait_for_device(timeout_s=5.0, poll_s=0.5)
    except MakcuError as e:
        pytest.skip(f"no MAKCU detected: {e}")
    assert port and baud in (4_000_000, 115_200)
