"""Hardware integration — safe queries + net-zero micro-move. Skipped if no device."""

import pytest

serial = pytest.importorskip("serial")

from makcu_engine import MakcuTransport, Mouse
from makcu_engine.types import Button, ButtonOwnership


def test_hardware_probes():
    try:
        port, baud = MakcuTransport.detect()
    except Exception as e:
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
