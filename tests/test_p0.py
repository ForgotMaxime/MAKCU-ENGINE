"""P0 fixes (MockTransport only, no hardware).

Covers: button stream state modes 1-3, BatchBuilder additions, mock/real
signature parity, MakcuProtocolError unification, click hold_ms=0,
ownership tri-state, click_human_like rng, and small aliases/exports.
"""

import inspect
import random
from unittest import mock

import pytest

from makcu_engine import CLICK_PROFILES, CoordSpace
from makcu_engine.abc import Transport
from makcu_engine.errors import MakcuProtocolError
from makcu_engine.mock import MockTransport
from makcu_engine.mouse import Mouse
from makcu_engine.transport import MakcuTransport
from makcu_engine.types import (
    Button,
    ButtonOwnership,
    Monitor,
    scale_coordinates,
    to_virtual,
    virtual_bounds,
)


def test_button_stream_state_all_modes() -> None:
    for raw, expected in (("0", False), ("1", True), ("2", True), ("3", True)):
        t = MockTransport(replies={"km.buttons": raw})
        assert Mouse(t).button_stream_state() is expected


def test_button_stream_state_start_stop_modes() -> None:
    t = MockTransport(replies={"km.buttons": "2"})
    m = Mouse(t)
    m.start_button_stream(2)
    assert m.button_stream_state() is True
    m.stop_button_stream()
    t.replies["km.buttons"] = "0"
    assert m.button_stream_state() is False


def test_batch_set_echo() -> None:
    t = MockTransport()
    n = Mouse(t).batch().set_echo(True).set_echo(False).execute()
    assert n == 2
    assert t.sent == ["km.echo(1)\rkm.echo(0)\r"]


def test_batch_move_bezier() -> None:
    t = MockTransport()
    Mouse(t).batch().move_bezier(100, 50).execute()
    assert t.sent == ["km.move(100,50,20,50,25)\r"]
    t2 = MockTransport()
    Mouse(t2).batch().move_bezier(100, 50, segments=10, ctrl_x=7, ctrl_y=-3).execute()
    assert t2.sent == ["km.move(100,50,10,7,-3)\r"]
    with pytest.raises(MakcuProtocolError):
        Mouse(MockTransport()).batch().move_bezier(10, 10, segments=0)
    with pytest.raises(MakcuProtocolError):
        Mouse(MockTransport()).batch().move_bezier(10, 10, ctrl_x=32768)


def test_batch_press_release_aliases() -> None:
    t = MockTransport()
    Mouse(t).batch().press(Button.LEFT).release(Button.LEFT).execute()
    assert t.sent == ["km.left(1)\rkm.left(0)\r"]
    t2 = MockTransport()
    Mouse(t2).batch().press(Button.SIDE1).release(Button.SIDE1, True).execute()
    assert t2.sent == ["km.ms1(1)\rkm.ms1(2)\r"]


def test_batch_set_screen_confirm() -> None:
    m = Mouse(MockTransport())
    with pytest.raises(MakcuProtocolError):
        m.batch().set_screen(1920, 1080)
    t = MockTransport()
    n = Mouse(t).batch().set_screen(1920, 1080, confirm=True).execute()
    assert n == 1
    assert t.sent == ["km.screen(1920,1080)\r"]
    with pytest.raises(MakcuProtocolError):
        Mouse(MockTransport()).batch().set_screen(0, 1080, confirm=True)


def test_batch_scroll_alias() -> None:
    t = MockTransport()
    Mouse(t).batch().scroll(3).execute()
    assert t.sent == ["km.wheel(3)\r"]


def test_mock_signature_parity_kwargs() -> None:
    t = MockTransport()
    assert t.query("km.version", retries=1) == "km.MAKCU"
    t.set("km.move", 1, 0, error_window_s=0.08)
    t.send_raw(b"km.wheel(1)\r", error_window_s=0.08)
    assert "km.move(1,0)" in t.sent
    # Same kwargs exist on the real transport (signature parity).
    for meth, kw in (
        (MakcuTransport.query, "retries"),
        (MakcuTransport.set, "error_window_s"),
        (MakcuTransport.send_raw, "error_window_s"),
    ):
        assert kw in inspect.signature(meth).parameters
    for meth, kw in (
        (MockTransport.query, "retries"),
        (MockTransport.set, "error_window_s"),
        (MockTransport.send_raw, "error_window_s"),
    ):
        assert kw in inspect.signature(meth).parameters


def test_abc_transport_has_parity_methods() -> None:
    for name in (
        "connect",
        "close",
        "reconnect",
        "ensure_connected",
        "on_connection_change",
        "remove_connection_callback",
        "get_stats",
        "reset_stats",
        "get_recent_lines",
        "no_wait",
        "enable_profiling",
        "disable_profiling",
        "profiling_enabled",
        "get_profile",
        "reset_profile",
    ):
        assert hasattr(Transport, name), name
    assert "retries" in inspect.signature(Transport.query).parameters
    assert "error_window_s" in inspect.signature(Transport.set).parameters


def test_mock_note_tx_binary_fallback() -> None:
    t = MockTransport()
    t.send_raw(b"\xff\xfe\r", error_window_s=0)
    lines = t.get_recent_lines()
    assert "TX b'" in lines[-1]


def test_mock_ensure_connected_and_connect_retries() -> None:
    t = MockTransport()
    t.connect(retries=2)
    assert t.is_connected is True
    t.ensure_connected()
    assert t.is_connected is True
    t.close()
    t.ensure_connected()


def test_wait_idle_negative_poll() -> None:
    with pytest.raises(MakcuProtocolError, match=r"poll_s.*:"):
        Mouse(MockTransport()).wait_idle(timeout_s=0.2, poll_s=-0.01)


def test_drag_abs_duration_cap() -> None:
    m = Mouse(MockTransport())
    with pytest.raises(MakcuProtocolError, match=r"duration.*:"):
        with mock.patch("time.sleep"):
            m.drag_abs((0, 0), (5, 5), duration=1e9)
    with pytest.raises(MakcuProtocolError, match=r"duration.*:"):
        with mock.patch("time.sleep"):
            m.drag_abs((0, 0), (5, 5), duration=61.0)


def test_monitor_scale_zero() -> None:
    bad = Monitor(x=0, y=0, w=100, h=100, scale=0.0)
    with pytest.raises(MakcuProtocolError, match=r"scale.*:"):
        to_virtual(10, 10, bad)
    with pytest.raises(MakcuProtocolError, match=r"scale.*:"):
        virtual_bounds([bad])
    with pytest.raises(MakcuProtocolError, match=r"scale.*:"):
        virtual_bounds([Monitor(x=0, y=0, w=100, h=100, scale=-2.0)])


def test_scale_coordinates_protocol_error() -> None:
    for kwargs in (
        {"game_dpi": 0, "desktop_dpi": 800, "sens": 1.0},
        {"game_dpi": 800, "desktop_dpi": 0, "sens": 1.0},
        {"game_dpi": 800, "desktop_dpi": 800, "sens": -1.0},
    ):
        with pytest.raises(MakcuProtocolError, match=r".*dpi|sens.*:"):
            scale_coordinates(10, 10, **kwargs)
    # MakcuProtocolError stays a ValueError for stdlib compat.
    with pytest.raises(ValueError):
        scale_coordinates(10, 10, game_dpi=0, desktop_dpi=800, sens=1.0)


def test_virtual_bounds_empty_protocol_error() -> None:
    with pytest.raises(MakcuProtocolError):
        virtual_bounds([])
    with pytest.raises(ValueError):
        virtual_bounds([])


def test_click_hold_zero_passthrough() -> None:
    t = MockTransport()
    Mouse(t).click(Button.LEFT, 2, 0)
    assert t.sent == ["km.click(1,2,0)"]
    t2 = MockTransport()
    Mouse(t2).batch().click(Button.RIGHT, 1, 0).execute()
    assert t2.sent == ["km.click(2,1,0)\r"]
    m = Mouse(MockTransport())
    for bad in (-1, 5001, 6000):
        with pytest.raises(MakcuProtocolError, match=r"hold_ms.*:"):
            m.click(Button.LEFT, 1, bad)
    with pytest.raises(MakcuProtocolError):
        m.click(Button.LEFT, 0, 0)


def test_ownership_all_four() -> None:
    t = MockTransport(
        replies={
            "km.left": "0",
            "km.right": "1",
            "km.middle": "2",
            "km.ms1": "3",
            "km.ms2": "0",
        }
    )
    m = Mouse(t)
    assert m.button_state(Button.LEFT) is ButtonOwnership.NONE
    assert m.button_state(Button.RIGHT) is ButtonOwnership.PHYSICAL
    assert m.button_state(Button.MIDDLE) is ButtonOwnership.INJECTED
    assert m.button_state(Button.SIDE1) is ButtonOwnership.BOTH
    states = m.get_button_states()
    assert states[Button.SIDE1] is ButtonOwnership.BOTH
    assert m.is_pressed(Button.LEFT) is False
    assert m.is_pressed(Button.RIGHT) is False
    assert m.is_pressed(Button.MIDDLE) is True
    assert m.is_pressed(Button.SIDE1) is True


def test_click_human_like_rng_seed() -> None:
    def run(seed: int) -> tuple[list[str], list[float]]:
        t = MockTransport()
        m = Mouse(t)
        sleeps: list[float] = []
        with mock.patch("time.sleep", side_effect=lambda s: sleeps.append(s)):
            m.click_human_like(Button.LEFT, count=3, profile="normal", rng=random.Random(seed))
        return list(t.sent), list(sleeps)

    sent_a, sleeps_a = run(42)
    sent_b, sleeps_b = run(42)
    assert sent_a == sent_b
    assert sleeps_a == sleeps_b
    assert sent_a == ["km.left(1)", "km.left(0)"] * 3
    sent_c, _ = run(43)
    assert sent_c == sent_a  # no jitter: same wire regardless of seed
    # Jitter path diverges per seed.
    t1, t2 = MockTransport(), MockTransport()
    with mock.patch("time.sleep"):
        Mouse(t1).click_human_like(Button.LEFT, jitter=5, rng=random.Random(1))
        Mouse(t2).click_human_like(Button.LEFT, jitter=5, rng=random.Random(2))
    moves1 = [c for c in t1.sent if c.startswith("km.move(")]
    moves2 = [c for c in t2.sent if c.startswith("km.move(")]
    assert len(moves1) == len(moves2) == 1


def test_click_human_like_default_rng() -> None:
    t = MockTransport()
    with mock.patch("time.sleep"), mock.patch("random.uniform", return_value=80.0):
        Mouse(t).click_human_like(Button.RIGHT, count=1)
    assert t.sent == ["km.right(1)", "km.right(0)"]


def test_scroll_alias() -> None:
    t = MockTransport()
    Mouse(t).scroll(-2)
    assert t.sent == ["km.wheel(-2)"]


def test_udp_relay_five_buttons() -> None:
    import importlib.util
    from pathlib import Path

    path = Path(__file__).resolve().parents[1] / "examples" / "udp_relay.py"
    spec = importlib.util.spec_from_file_location("udp_relay_p0", path)
    assert spec is not None and spec.loader is not None
    udp = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(udp)
    assert set(udp._BUTTONS) == {"LEFT", "RIGHT", "MIDDLE", "SIDE1", "SIDE2"}
    for name in ("LEFT", "RIGHT", "MIDDLE", "SIDE1", "SIDE2"):
        assert udp.parse_datagram(f"PRESS:{name}") is not None
        assert udp.parse_datagram(f"CLICK:{name}") is not None


def test_package_exports() -> None:
    import makcu_engine

    assert makcu_engine.CLICK_PROFILES is CLICK_PROFILES
    assert makcu_engine.CoordSpace is CoordSpace
    assert "CLICK_PROFILES" in makcu_engine.__all__
    assert "CoordSpace" in makcu_engine.__all__


def test_lock_vs_press_docstrings() -> None:
    assert "is_pressed" in (Mouse.is_locked.__doc__ or "")
    assert "is_locked" in (Mouse.is_pressed.__doc__ or "")
    assert "LockTarget" in (Mouse.is_locked.__doc__ or "")
    assert "Button" in (Mouse.is_pressed.__doc__ or "")


def test_enable_button_monitoring_alias() -> None:
    t = MockTransport()
    m = Mouse(t)
    m.enable_button_monitoring(2)
    assert "km.buttons(2)" in t.sent
    m.stop_button_stream()
    assert "km.buttons(0)" in t.sent
