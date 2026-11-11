"""P1b: message convention, api gate, move_abs/pattern/sequence (MockTransport only)."""

import importlib.util
import json
from pathlib import Path
from unittest import mock

import pytest

from makcu_engine.errors import MakcuProtocolError
from makcu_engine.mock import MockTransport
from makcu_engine.mouse import Mouse
from makcu_engine.types import Button

EXAMPLES = Path(__file__).resolve().parents[1] / "examples"


def _load_example(name):
    path = EXAMPLES / f"{name}.py"
    spec = importlib.util.spec_from_file_location(f"examples_{name}", path)
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class _ConvergingTransport(MockTransport):
    """Mock getpos that tracks km.move deltas from a start position."""

    def __init__(self, x: int = 0, y: int = 0) -> None:
        super().__init__()
        self.pos = [x, y]

    def query(self, name: str, *args: int | str, retries: int = 1) -> str:
        if name == "km.getpos":
            return f"km.getpos({self.pos[0]},{self.pos[1]})"
        return super().query(name, *args, retries=retries)

    def set(self, name: str, *args: int | str, error_window_s: float = 0.08) -> None:
        if name == "km.move" and len(args) >= 2:
            self.pos[0] += int(args[0])
            self.pos[1] += int(args[1])
        return super().set(name, *args, error_window_s=error_window_s)


def test_p1b_confirm_prefix() -> None:
    with pytest.raises(MakcuProtocolError, match=r"confirm.*:"):
        Mouse(MockTransport()).set_screen(1920, 1080)


def test_p1b_move_smooth_prefixes() -> None:
    m = Mouse(MockTransport())
    with pytest.raises(MakcuProtocolError, match=r"steps.*:"):
        m.move_smooth(10, 0, steps=0)
    with pytest.raises(MakcuProtocolError, match=r"step_s.*:"):
        m.move_smooth(10, 0, steps=5, step_s=-0.1)


def test_p1b_duration_gap_prefixes() -> None:
    m = Mouse(MockTransport())
    with (
        mock.patch("time.sleep"),
        pytest.raises(MakcuProtocolError, match=r"duration.*:"),
    ):
        m.drag_abs((0, 0), (5, 5), duration=0)
    with pytest.raises(MakcuProtocolError, match=r"gap_s.*:"):
        m.double_click(Button.LEFT, gap_s=-0.01)


def test_p1b_profile_count_jitter_prefixes() -> None:
    m = Mouse(MockTransport())
    with (
        mock.patch("time.sleep"),
        mock.patch("random.uniform", return_value=50.0),
        pytest.raises(MakcuProtocolError, match=r"profile.*:"),
    ):
        m.click_human_like(Button.LEFT, profile="nope")
    with pytest.raises(MakcuProtocolError, match=r"count.*:"):
        m.click_human_like(Button.LEFT, count=0)
    with pytest.raises(MakcuProtocolError, match=r"jitter.*:"):
        m.click_human_like(Button.LEFT, jitter=-1)


def test_p1b_read_exact_n_prefix() -> None:
    from makcu_engine.transport import MakcuTransport

    t = MakcuTransport("COM9", 115200)
    t._ser = object()  # connected enough: n check runs before any I/O
    with pytest.raises(MakcuProtocolError, match=r"n.*:"):
        t.read_exact(0)


def test_p1b_monitors_prefix() -> None:
    from makcu_engine.types import virtual_bounds

    with pytest.raises(MakcuProtocolError, match=r"monitors.*:"):
        virtual_bounds([])


def test_p1b_macros_prefixes(tmp_path) -> None:
    macros = _load_example("macros")
    assert issubclass(MakcuProtocolError, ValueError)
    with pytest.raises(MakcuProtocolError, match=r"sx.*:"):
        macros.scale_xy([{"t_ms": 0, "x": 1, "y": 1}], 0.0, 1.0)
    with pytest.raises(MakcuProtocolError, match=r"sy.*:"):
        macros.scale_xy([{"t_ms": 0, "x": 1, "y": 1}], 1.0, -2.0)
    with pytest.raises(MakcuProtocolError, match=r"factor.*:"):
        macros.adjust_delays([{"t_ms": 0, "x": 0, "y": 0}], 10.0)
    with pytest.raises(MakcuProtocolError, match=r"events.*:"):
        macros.to_rel_deltas([])


def _write_macro(tmp_path, payload) -> Path:
    p = tmp_path / "m.json"
    p.write_text(json.dumps(payload), encoding="utf-8")
    return p


def test_p1b_macros_load_prefixes(tmp_path) -> None:
    macros = _load_example("macros")
    with pytest.raises(MakcuProtocolError, match=r"macro.*:"):
        macros.load_macro(_write_macro(tmp_path, [1, 2, 3]))
    with pytest.raises(MakcuProtocolError, match=r"events.*:"):
        macros.load_macro(_write_macro(tmp_path, {"version": 1, "events": []}))
    with pytest.raises(MakcuProtocolError, match=r"event.*:"):
        macros.load_macro(_write_macro(tmp_path, {"version": 1, "events": ["oops"]}))
    with pytest.raises(MakcuProtocolError, match=r"x.*:"):
        macros.load_macro(_write_macro(tmp_path, {"version": 1, "events": [{"t_ms": 0}]}))
    with pytest.raises(MakcuProtocolError, match=r"t_ms.*:"):
        bad = {"version": 1, "events": [{"t_ms": -1, "x": 0, "y": 0}]}
        macros.load_macro(_write_macro(tmp_path, bad))
    with pytest.raises(MakcuProtocolError, match=r"x.*:"):
        bad = {"version": 1, "events": [{"t_ms": 0, "x": 40000, "y": 0}]}
        macros.load_macro(_write_macro(tmp_path, bad))


def test_p1b_requires_api_gate() -> None:
    m = Mouse(MockTransport())
    m.requires_api(0)
    m.requires_api(1)
    with pytest.raises(MakcuProtocolError, match=r"api_version.*:") as exc:
        m.requires_api(2)
    assert "1" in str(exc.value) and "2" in str(exc.value)
    with pytest.raises(MakcuProtocolError, match=r"n.*:"):
        m.requires_api(-1)


def test_p1b_gated_paths_new_firmware() -> None:
    t = MockTransport()
    m = Mouse(t)
    m.set_autorelease(5000)
    m.heartbeat()
    assert "km.autorelease(5000)" in t.sent
    assert "km.heartbeat()" in t.sent
    with pytest.raises(MakcuProtocolError, match=r"autorelease.*:"):
        m.set_autorelease(60001)


def test_p1b_gated_paths_old_firmware() -> None:
    m = Mouse(MockTransport(replies={"km.api_version": "ERR"}))
    with pytest.raises(MakcuProtocolError, match=r"api_version.*:"):
        m.set_autorelease(100)
    with pytest.raises(MakcuProtocolError, match=r"api_version.*:"):
        m.heartbeat()


def test_p1b_move_abs_full_jump() -> None:
    t = _ConvergingTransport(0, 0)
    m = Mouse(t)
    with mock.patch("time.sleep") as slp:
        m.move_abs((10, 0), speed=1.0, wait_ms=2)
    assert t.sent == ["km.move(10,0)"]
    assert [c.args[0] for c in slp.call_args_list] == [0.002]


def test_p1b_move_abs_fractional_speed() -> None:
    t = _ConvergingTransport(0, 0)
    with mock.patch("time.sleep"):
        Mouse(t).move_abs((8, 0), speed=0.5, wait_ms=2)
    assert t.sent == ["km.move(4,0)", "km.move(2,0)", "km.move(1,0)"]


def test_p1b_move_abs_clamps_and_stops() -> None:
    t = _ConvergingTransport(0, 0)
    with mock.patch("time.sleep") as slp:
        Mouse(t).move_abs((1, 1), speed=5.0, wait_ms=2)
    assert t.sent == []  # anti-overflick guard: |dx|,|dy| <= 1 already
    assert slp.call_count == 0
    t2 = _ConvergingTransport(0, 0)
    with mock.patch("time.sleep"):
        Mouse(t2).move_abs((4, 0), speed=9.0, wait_ms=0)
    assert t2.sent == ["km.move(4,0)"]  # speed clamped to 1.0


def test_p1b_move_abs_min_progress() -> None:
    t = _ConvergingTransport(0, 0)
    with mock.patch("time.sleep"):
        Mouse(t).move_abs((5, 0), speed=0.1, wait_ms=0)
    assert t.sent == ["km.move(1,0)"] * 4  # tiny factor still converges 1px/step


def test_p1b_move_abs_validation() -> None:
    m = Mouse(MockTransport())
    with pytest.raises(MakcuProtocolError, match=r"speed.*:"):
        m.move_abs((5, 5), speed=0)
    with pytest.raises(MakcuProtocolError, match=r"speed.*:"):
        m.move_abs((5, 5), speed=-1.0)
    with pytest.raises(MakcuProtocolError, match=r"wait_ms.*:"):
        m.move_abs((5, 5), wait_ms=-1)
    with pytest.raises(MakcuProtocolError, match=r"x.*:"):
        m.move_abs((32768, 0))


def test_p1b_move_pattern_wire_and_sleep() -> None:
    t = MockTransport()
    with mock.patch("time.sleep") as slp:
        Mouse(t).move_pattern([(5, 0), (-2, 3)], step_ms=5)
    assert t.sent == ["km.move(5,0)", "km.move(-2,3)"]
    assert [c.args[0] for c in slp.call_args_list] == [0.005]
    t2 = MockTransport()
    with mock.patch("time.sleep") as slp2:
        Mouse(t2).move_pattern([(1, 1)], step_ms=0)
    assert t2.sent == ["km.move(1,1)"]
    assert slp2.call_count == 0


def test_p1b_move_pattern_validation() -> None:
    m = Mouse(MockTransport())
    with pytest.raises(MakcuProtocolError, match=r"points.*:"):
        m.move_pattern([])
    with pytest.raises(MakcuProtocolError, match=r"step_ms.*:"):
        m.move_pattern([(1, 1)], step_ms=-1)
    with pytest.raises(MakcuProtocolError, match=r"points entry.*:"):
        m.move_pattern([(1, 2, 3)])


def test_p1b_click_sequence_wire() -> None:
    t = MockTransport()
    Mouse(t).click_sequence(Button.LEFT, [10, 20])
    assert t.sent == ["km.click(1,1,10)", "km.click(1,1,20)"]
    t2 = MockTransport()
    Mouse(t2).click_sequence(Button.RIGHT, [0])
    assert t2.sent == ["km.click(2,1,0)"]  # 0 = firmware-random hold, like click()


def test_p1b_click_sequence_validation() -> None:
    m = Mouse(MockTransport())
    with pytest.raises(MakcuProtocolError, match=r"holds_ms.*:"):
        m.click_sequence(Button.LEFT, [])
    with pytest.raises(MakcuProtocolError, match=r"holds_ms entry.*:"):
        m.click_sequence(Button.LEFT, ["x"])
    with pytest.raises(MakcuProtocolError, match=r"hold_ms.*:"):
        m.click_sequence(Button.LEFT, [-5])


def test_p1b_up_silent_release_is_not_a_force() -> None:
    """Verdict: wire 2 skips the release frame (protocol.md), so it cannot unstick.

    No force variant exists in the ASCII protocol: plain up() (wire 0) is the
    unsticking primitive, and init() already best-efforts it on all buttons.
    """
    t = MockTransport()
    m = Mouse(t)
    m.up(Button.LEFT)
    m.up(Button.LEFT, silent_release=True)
    assert t.sent == ["km.left(0)", "km.left(2)"]
