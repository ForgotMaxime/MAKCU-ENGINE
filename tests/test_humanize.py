"""Humanize features: bezier, human-like clicks, abs drag, coord scaling."""

from unittest import mock

import pytest

from makcu_engine.errors import MakcuProtocolError
from makcu_engine.mock import MockTransport
from makcu_engine.mouse import CLICK_PROFILES, Mouse
from makcu_engine.types import Button, CoordSpace, scale_coordinates


def test_move_bezier_default_ctrl():
    t = MockTransport()
    m = Mouse(t)
    m.move_bezier(100, 50)
    assert t.sent == ["km.move(100,50,20,50,25)"]


def test_move_bezier_custom_args():
    t = MockTransport()
    m = Mouse(t)
    m.move_bezier(100, 50, segments=10, ctrl_x=7, ctrl_y=-3)
    assert t.sent == ["km.move(100,50,10,7,-3)"]


@pytest.mark.parametrize("segments", [0, 256, -1])
def test_move_bezier_bad_segments(segments):
    m = Mouse(MockTransport())
    with pytest.raises(MakcuProtocolError):
        m.move_bezier(10, 10, segments=segments)


@pytest.mark.parametrize("ctrl", [32768, -32769])
def test_move_bezier_bad_ctrl(ctrl):
    m = Mouse(MockTransport())
    with pytest.raises(MakcuProtocolError):
        m.move_bezier(10, 10, ctrl_x=ctrl)
    with pytest.raises(MakcuProtocolError):
        m.move_bezier(10, 10, ctrl_y=ctrl)


def test_click_profiles_table():
    assert CLICK_PROFILES == {
        "normal": (60, 120, 100, 180),
        "fast": (30, 60, 50, 100),
        "slow": (100, 180, 150, 300),
        "variable": (40, 200, 80, 250),
        "gaming": (20, 40, 30, 60),
    }


def test_click_human_like_wire_and_timing():
    t = MockTransport()
    m = Mouse(t)
    uniforms = []
    sleeps = []

    def fake_uniform(a, b):
        uniforms.append((a, b))
        return (a + b) / 2

    with (
        mock.patch("random.uniform", side_effect=fake_uniform),
        mock.patch("time.sleep", side_effect=lambda s: sleeps.append(s)),
    ):
        m.click_human_like(Button.LEFT, count=2, profile="normal")

    assert t.sent == ["km.left(1)", "km.left(0)", "km.left(1)", "km.left(0)"]
    assert uniforms == [(60, 120), (100, 180), (60, 120)]
    assert sleeps == [0.09, 0.14, 0.09]


def test_click_human_like_jitter_moves():
    t = MockTransport()
    m = Mouse(t)
    with (
        mock.patch("random.randint", side_effect=[3, -2, 1, 1]),
        mock.patch("random.uniform", return_value=100.0),
        mock.patch("time.sleep"),
    ):
        m.click_human_like(Button.RIGHT, count=2, profile="fast", jitter=5)
    moves = [c for c in t.sent if c.startswith("km.move(")]
    assert moves == ["km.move(3,-2)", "km.move(1,1)"]
    assert "km.right(1)" in t.sent
    assert "km.right(0)" in t.sent


@pytest.mark.parametrize(
    "kwargs",
    [
        {"profile": "nope"},
        {"count": 0},
        {"count": 256},
        {"jitter": -1},
    ],
)
def test_click_human_like_validation(kwargs):
    m = Mouse(MockTransport())
    with (
        mock.patch("random.uniform", return_value=50.0),
        mock.patch("time.sleep"),
        pytest.raises(MakcuProtocolError),
    ):
        m.click_human_like(Button.LEFT, **kwargs)


def test_drag_abs_wire():
    t = MockTransport()
    m = Mouse(t)
    with mock.patch("time.sleep"):
        m.drag_abs((100, 200), (130, 200), duration=0.5)
    segments = max(10, int(0.5 * 30))
    assert t.sent[0] == "km.moveto(100,200)"
    assert t.sent[1] == "km.left(1)"
    assert t.sent[-1] == "km.left(0)"
    moves = [c for c in t.sent if c.startswith("km.move(")]
    assert len(moves) == segments
    total_dx = sum(int(c[len("km.move(") : -1].split(",")[0]) for c in moves)
    total_dy = sum(int(c[len("km.move(") : -1].split(",")[1]) for c in moves)
    assert (total_dx, total_dy) == (30, 0)


def test_drag_abs_duration_minimum_segments():
    t = MockTransport()
    m = Mouse(t)
    with mock.patch("time.sleep"):
        m.drag_abs((0, 0), (5, 5), duration=0.05)
    moves = [c for c in t.sent if c.startswith("km.move(")]
    assert len(moves) == 10


@pytest.mark.parametrize("duration", [0, -1.0])
def test_drag_abs_bad_duration(duration):
    m = Mouse(MockTransport())
    with pytest.raises(MakcuProtocolError):
        m.drag_abs((0, 0), (5, 5), duration=duration)


def test_scale_coordinates():
    assert scale_coordinates(100, 50, game_dpi=800, desktop_dpi=800, sens=1.0) == (
        100,
        50,
    )
    assert scale_coordinates(100, 0, game_dpi=1600, desktop_dpi=800, sens=0.5) == (
        100,
        0,
    )
    assert scale_coordinates(3, 3, game_dpi=800, desktop_dpi=1600, sens=2.0) == (3, 3)
    assert CoordSpace.DESKTOP.value == "desktop"
    assert CoordSpace.GAME.value == "game"


@pytest.mark.parametrize(
    "kwargs",
    [
        {"game_dpi": 0, "desktop_dpi": 800, "sens": 1.0},
        {"game_dpi": 800, "desktop_dpi": 0, "sens": 1.0},
        {"game_dpi": 800, "desktop_dpi": 800, "sens": -1.0},
    ],
)
def test_scale_coordinates_validation(kwargs):
    with pytest.raises(ValueError):
        scale_coordinates(10, 10, **kwargs)
