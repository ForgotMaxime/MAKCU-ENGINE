"""Transport observability (MockTransport only) + pure monitor geometry."""

import pytest

from makcu_engine.mock import MockTransport
from makcu_engine.types import (
    Bounds,
    Monitor,
    remap_point,
    to_local,
    to_virtual,
    virtual_bounds,
)


def test_stats_increment_all_paths():
    t = MockTransport()
    t.query("km.version")
    t.set("km.move", 10, 0)
    t.send_raw(b"km.wheel(1)\r")
    t.fire_and_forget("km.move", 1, 1)
    s = t.get_stats()
    assert s["cmds_sent"] == 4
    assert s["tx_bytes"] == len(b"km.version()\rkm.move(10,0)\rkm.wheel(1)\rkm.move(1,1)\r")
    assert s["avg_rtt_ms"] >= 0.0
    assert s["reconnects"] == 0
    assert s["last_error"] is None


def test_stats_reset_and_error():
    from makcu_engine.errors import MakcuProtocolError

    t = MockTransport()
    t.query("km.version")
    t.fail_next_send = True
    with pytest.raises(MakcuProtocolError):
        t.set("km.move", 1, 0)
    s = t.get_stats()
    assert s["cmds_sent"] == 1  # the failed set never transmitted
    assert "km.move" in str(s["last_error"])
    t.reconnect()
    assert t.get_stats()["reconnects"] == 1
    t.reset_stats()
    assert t.get_stats() == {
        "tx_bytes": 0,
        "cmds_sent": 0,
        "reconnects": 0,
        "avg_rtt_ms": 0.0,
        "last_error": None,
    }


def test_ring_buffer_bounded():
    t = MockTransport()
    for i in range(250):
        t.fire_and_forget("km.move", i, 0)
    lines = t.get_recent_lines(500)
    assert len(lines) == 200
    assert lines[0] == "TX km.move(50,0)\r"
    assert lines[-1] == "TX km.move(249,0)\r"
    assert t.get_recent_lines() == lines[-20:]
    assert t.get_recent_lines(3) == lines[-3:]


def test_ring_buffer_query_set_contents():
    t = MockTransport()
    t.query("km.version")
    t.set("km.move", 1, 2)
    lines = t.get_recent_lines()
    assert lines[0] == "TX km.version()\r"
    assert lines[1] == "RX km.MAKCU"
    assert lines[2] == "TX km.move(1,2)\r"


def test_no_wait_set_skips_err_window():
    from makcu_engine.errors import MakcuProtocolError

    t = MockTransport()
    t.fail_next_send = True
    with t.no_wait():
        t.set("km.move", 1, 0)  # must not raise
    assert "km.move(1,0)" in t.sent
    # Outside the block the ERR window still applies.
    t.fail_next_send = True
    with pytest.raises(MakcuProtocolError):
        t.set("km.move", 1, 0)
    # Nesting: still no-wait until the outer block exits.
    t.fail_next_send = True
    with t.no_wait(), t.no_wait():
        t.set("km.move", 2, 0)
    assert "km.move(2,0)" in t.sent


def test_no_wait_queries_still_wait():
    t = MockTransport()
    with t.no_wait():
        assert t.query("km.version") == "km.MAKCU"
        assert t.query("km.getpos") == "km.getpos(100,200)"
    s = t.get_stats()
    assert s["cmds_sent"] == 2
    assert len(t.get_recent_lines()) == 4  # TX+RX per query


def test_virtual_bounds_negative_origins():
    ms = [Monitor(x=0, y=0, w=1920, h=1080), Monitor(x=-1920, y=100, w=1920, h=1080)]
    assert virtual_bounds(ms) == Bounds(x=-1920, y=0, w=3840, h=1180)
    assert virtual_bounds([Monitor(x=5, y=7, w=100, h=50)]) == Bounds(x=5, y=7, w=100, h=50)
    with pytest.raises(ValueError):
        virtual_bounds([])


def test_point_conversions_roundtrip():
    m = Monitor(x=-1920, y=100, w=1920, h=1080)
    assert to_virtual(0, 0, m).x == -1920
    assert to_virtual(1920, 1080, m).y == 1180
    assert to_local(-1920, 100, m).x == 0
    v = to_virtual(320, 240, m)
    assert (to_local(v.x, v.y, m).x, to_local(v.x, v.y, m).y) == (320, 240)


def test_point_conversions_scaled():
    m = Monitor(x=0, y=0, w=3840, h=2160, scale=2.0)
    v = to_virtual(3840, 2160, m)
    assert (v.x, v.y) == (1920, 1080)
    assert virtual_bounds([m]) == Bounds(x=0, y=0, w=1920, h=1080)


def test_remap_point_across_monitors():
    src = Monitor(x=0, y=0, w=1920, h=1080)
    dst = Monitor(x=1920, y=0, w=1920, h=1080)
    p = remap_point(100, 200, src, dst)
    assert (p.x, p.y) == (-1820, 200)  # same virtual point, dst-local
    assert to_virtual(p.x, p.y, dst) == to_virtual(100, 200, src)
