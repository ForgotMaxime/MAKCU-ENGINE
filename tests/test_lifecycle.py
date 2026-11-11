from makcu_engine.errors import MakcuProtocolError
from makcu_engine.mock import MockTransport
from makcu_engine.mouse import Mouse
from makcu_engine.types import Button


def test_init_releases_all_buttons():
    t = MockTransport()
    Mouse(t).init()
    for wire in ("km.left(0)", "km.right(0)", "km.middle(0)", "km.ms1(0)", "km.ms2(0)"):
        assert wire in t.sent


def test_init_stops_stream_when_running():
    t = MockTransport()
    m = Mouse(t)
    m.start_button_stream(1)
    m.init()
    assert "km.buttons(0)" in t.sent


def test_init_without_stream_sends_no_buttons_off():
    t = MockTransport()
    Mouse(t).init()
    assert "km.buttons(0)" not in t.sent


def test_init_never_raises():
    class Flaky(MockTransport):
        def set(self, name: str, *args: int | str) -> None:
            raise MakcuProtocolError(f"{name} failed")

    m = Mouse(Flaky())
    m._stream_mode = 1  # pretend a stream is running; stop will also fail
    m.init()  # must swallow every individual error


def test_press_release_wire_names():
    t = MockTransport()
    m = Mouse(t)
    m.press(Button.LEFT)
    m.release(Button.LEFT)
    m.press(Button.SIDE1)
    m.release(Button.SIDE1)
    m.press(Button.SIDE2)
    m.release(Button.SIDE2, silent_release=True)
    assert "km.left(1)" in t.sent
    assert "km.left(0)" in t.sent
    assert "km.ms1(1)" in t.sent
    assert "km.ms1(0)" in t.sent
    assert "km.ms2(1)" in t.sent
    assert "km.ms2(2)" in t.sent


def test_double_click_sends_two_clicks_no_sleep(monkeypatch):
    import time

    calls: list[float] = []
    monkeypatch.setattr(time, "sleep", calls.append)
    t = MockTransport()
    Mouse(t).double_click(Button.LEFT, gap_s=0)
    assert t.sent == ["km.click(1,1)", "km.click(1,1)"]
    assert calls == []


def test_double_click_sleeps_once_between_clicks(monkeypatch):
    import time

    calls: list[float] = []
    monkeypatch.setattr(time, "sleep", calls.append)
    t = MockTransport()
    Mouse(t).double_click(Button.RIGHT, gap_s=0.05)
    assert t.sent == ["km.click(2,1)", "km.click(2,1)"]
    assert calls == [0.05]


def test_double_click_rejects_negative_gap():
    try:
        Mouse(MockTransport()).double_click(Button.LEFT, gap_s=-0.01)
        raise AssertionError("should raise")
    except MakcuProtocolError:
        pass


def test_batch_up_silent_flag():
    t = MockTransport()
    n = Mouse(t).batch().down(Button.LEFT).up(Button.LEFT, silent_release=True).execute()
    assert n == 2
    assert t.sent == ["km.left(1)\rkm.left(2)\r"]


def test_batch_up_default_is_plain_release():
    t = MockTransport()
    n = Mouse(t).batch().down(Button.SIDE1).up(Button.SIDE1).execute()
    assert n == 2
    assert t.sent == ["km.ms1(1)\rkm.ms1(0)\r"]
