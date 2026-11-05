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
