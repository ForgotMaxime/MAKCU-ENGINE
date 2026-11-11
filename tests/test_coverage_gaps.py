"""Coverage gaps: one named test per audit line (mock-only, no hardware)."""

import time
from unittest import mock

import pytest

from makcu_engine import protocol
from makcu_engine.errors import MakcuProtocolError, MakcuTimeoutError
from makcu_engine.events import ButtonMonitor
from makcu_engine.mock import MockTransport
from makcu_engine.mouse import Mouse
from makcu_engine.types import Button, LockTarget


def test_gap_monitor_survives_garbage_frame():
    t = MockTransport()
    mon = ButtonMonitor(Mouse(t), poll_s=0.01)
    seen: list[int] = []
    mon.on_change(seen.append)
    mon.start(1)
    try:
        assert mon.is_running
        t.push_stream_frame(b"garbage\r\n")
        t.push_stream_frame(b"km.\x02\r\n")
        deadline = time.monotonic() + 2.0
        while time.monotonic() < deadline:
            if mon.pump_errors >= 1 and (0x02 in seen or 0x02 in mon.drain()):
                break
            time.sleep(0.02)
        assert mon.pump_errors >= 1
        assert mon.is_running
        assert 0x02 in seen
        # Drain must still work under lock after errors.
        assert isinstance(mon.drain(), list)
    finally:
        mon.stop()
    assert not mon.is_running


def test_gap_monitor_restart_after_thread_death():
    t = MockTransport()
    mon = ButtonMonitor(Mouse(t), poll_s=0.01)
    mon.start(1)
    assert mon.is_running
    # Simulate thread death without stop(): signal exit and join directly,
    # leaving a dead thread object behind.
    mon._stop.set()
    assert mon._thread is not None
    mon._thread.join(2.0)
    assert not mon._thread.is_alive()
    # stop() must tolerate the dead thread, then start() must work again.
    mon.stop()
    assert not mon.is_running
    mon.start(1)
    try:
        assert mon.is_running
        assert mon.pump_errors >= 0
    finally:
        mon.stop()
    assert not mon.is_running


def test_gap_batch_click_with_hold():
    t = MockTransport()
    n = Mouse(t).batch().click(Button.LEFT, 2, 50).execute()
    assert n == 1
    assert t.sent == ["km.click(1,2,50)\r"]
    with pytest.raises(MakcuProtocolError):
        Mouse(MockTransport()).batch().click(Button.LEFT, 1, 6000)


def test_gap_set_lock_wire_and_batch():
    t = MockTransport()
    m = Mouse(t)
    m.set_lock(LockTarget.MX, True)
    m.set_lock(LockTarget.MX_POS, False)
    assert "km.lock_mx(1)" in t.sent
    assert "km.lock_mx+(0)" in t.sent
    t2 = MockTransport()
    n = Mouse(t2).batch().set_lock(LockTarget.MY, True).execute()
    assert n == 1
    assert t2.sent == ["km.lock_my(1)\r"]


def test_gap_events_stop_swallows_timeout():
    t = MockTransport()
    mon = ButtonMonitor(Mouse(t))
    mon.start(1)
    with mock.patch.object(mon._mouse, "stop_button_stream", side_effect=MakcuTimeoutError("t")):
        mon.stop()  # must not raise
    assert not mon.is_running


def test_gap_mock_profile_min_max_ordering():
    t = MockTransport()
    t.enable_profiling()
    t.query("km.version")
    t.query("km.version")
    t.set("km.move", 1, 0)
    prof = t.get_profile()
    for entry in prof.values():
        assert entry["min_ms"] <= entry["avg_ms"] <= entry["max_ms"]
        assert entry["min_ms"] >= 0.0
        assert entry["count"] >= 1


def test_gap_mouse_step_s_negative_rejected():
    with pytest.raises(MakcuProtocolError):
        Mouse(MockTransport()).move_smooth(10, 0, steps=5, step_s=-0.1)


def test_gap_mode3_loop_guard():
    t = MockTransport()
    m = Mouse(t)
    m.start_button_stream(3)
    t.start_event_loop()
    try:
        with pytest.raises(MakcuProtocolError, match="mode 3"):
            m.poll_button_event(0.05)
    finally:
        t.stop_event_loop()
        m.stop_button_stream()


def test_gap_protocol_non_ascii_reply():
    with pytest.raises(MakcuProtocolError, match="non-ascii"):
        protocol.strip_prompt(b"\xff\xfe\r\n>>> ")


def test_gap_protocol_empty_reply():
    with pytest.raises(MakcuProtocolError, match="empty reply"):
        protocol.strip_prompt(b"\r\n>>> ")
    with pytest.raises(MakcuProtocolError):
        protocol.strip_prompt(b">>> ")


def test_gap_protocol_screen_h_range():
    with pytest.raises(MakcuProtocolError):
        protocol.parse_screen("km.screen(1920,0)")
    with pytest.raises(MakcuProtocolError):
        protocol.parse_screen("km.screen(1920,32768)")
    assert protocol.parse_screen("km.screen(1920,1080)").height == 1080


def test_gap_protocol_crlf_variants():
    assert protocol.strip_prompt(b"0\r\n>>> ") == "0"
    assert protocol.strip_prompt(b"0\n>>> ") == "0"
    assert protocol.strip_prompt(b"km.left()\r\n0\r\n>>> ") == "0"


def test_gap_stream_timeouts():
    t = MockTransport()
    with pytest.raises(MakcuTimeoutError):
        t.read_line(0.01)
    with pytest.raises(MakcuTimeoutError):
        t.read_exact(1, 0.01)
    m = Mouse(t)
    m.start_button_stream(1)
    try:
        with pytest.raises(MakcuTimeoutError):
            m.poll_button_event(0.01)
    finally:
        m.stop_button_stream()


def test_gap_human_like_jitter_moves_bounded():
    t = MockTransport()
    m = Mouse(t)
    with (
        mock.patch("random.randint", side_effect=[2, -2, 1, 1]),
        mock.patch("random.uniform", return_value=50.0),
        mock.patch("time.sleep"),
    ):
        m.click_human_like(Button.LEFT, count=2, profile="fast", jitter=5)
    moves = [c for c in t.sent if c.startswith("km.move(")]
    assert moves == ["km.move(2,-2)", "km.move(1,1)"]


def test_gap_human_like_validation_beyond_happy_path():
    m = Mouse(MockTransport())
    for kwargs in (
        {"profile": "nope"},
        {"count": 0},
        {"count": 256},
        {"jitter": -1},
        {"jitter": -5},
    ):
        with (
            mock.patch("random.uniform", return_value=50.0),
            mock.patch("time.sleep"),
            pytest.raises(MakcuProtocolError),
        ):
            m.click_human_like(Button.LEFT, **kwargs)  # type: ignore[arg-type]
    # jitter=0 sends no moves but still clicks.
    t = MockTransport()
    with (
        mock.patch("random.uniform", return_value=50.0),
        mock.patch("time.sleep"),
    ):
        Mouse(t).click_human_like(Button.LEFT, count=1, jitter=0)
    assert t.sent == ["km.left(1)", "km.left(0)"]


def test_gap_drag_abs_releases_on_move_error():
    t = MockTransport()
    m = Mouse(t)
    with (
        mock.patch.object(Mouse, "move_smooth", side_effect=RuntimeError("boom")),
        mock.patch("time.sleep"),
        pytest.raises(RuntimeError),
    ):
        m.drag_abs((0, 0), (10, 10))
    assert "km.left(1)" in t.sent
    assert "km.left(0)" in t.sent


def test_gap_init_partial_failure_never_raises():
    class Partial(MockTransport):
        def set(self, name: str, *args: int | str) -> None:
            if name == "km.left":
                raise MakcuProtocolError(f"{name} failed")
            super().set(name, *args)

    m = Mouse(Partial())
    m._stream_mode = 1
    with mock.patch.object(Mouse, "stop_button_stream", side_effect=MakcuTimeoutError("t")):
        m.init()  # must swallow every individual error


def test_gap_double_click_side_buttons():
    t = MockTransport()
    with mock.patch("time.sleep") as slp:
        Mouse(t).double_click(Button.SIDE1, gap_s=0.05)
    assert t.sent == ["km.click(4,1)", "km.click(4,1)"]
    assert slp.call_count == 1
    t2 = MockTransport()
    with mock.patch("time.sleep"):
        Mouse(t2).double_click(Button.SIDE2, gap_s=0)
    assert t2.sent == ["km.click(5,1)", "km.click(5,1)"]


def test_gap_get_recent_lines_n_larger_than_buffer():
    t = MockTransport()
    t.query("km.version")
    assert len(t.get_recent_lines(1000)) == 2
    assert t.get_recent_lines(0) == []
    assert t.get_recent_lines(-3) == []
