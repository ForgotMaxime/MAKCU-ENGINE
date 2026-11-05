from makcu_engine.mock import MockTransport
from makcu_engine.mouse import Mouse
from makcu_engine.types import Button


def test_mouse_with_mock():
    t = MockTransport()
    m = Mouse(t)
    assert m.version() == "km.MAKCU"
    assert m.device().route == "MK"
    assert m.moving() is False
    assert m.getpos().x == 100
    m.move(10, -5)
    m.down(Button.LEFT)
    m.up(Button.LEFT)
    m.click(Button.LEFT, 2, 50)
    m.wheel(1)
    assert "km.move(10,-5)" in t.sent
    assert "km.left(1)" in t.sent
    assert "km.click(1,2,50)" in t.sent


def test_batch_with_mock():
    t = MockTransport()
    m = Mouse(t)
    n = m.batch().move(10, 0).move(-10, 0).wheel(1).execute()
    assert n == 3
    assert len(t.sent) == 1
    assert t.sent[0] == "km.move(10,0)\rkm.move(-10,0)\rkm.wheel(1)\r"


def test_batch_validates_like_mouse():
    from makcu_engine.errors import MakcuProtocolError

    t = MockTransport()
    m = Mouse(t)
    for build in (
        lambda: m.batch().click(Button.LEFT, 0),
        lambda: m.batch().click(Button.LEFT, 1, 6000),
        lambda: m.batch().moveto(-1, 0),
        lambda: m.batch().silent(0, 40000),
        lambda: m.batch().set_move_mask(0, 0, 0, 2),
        lambda: m.batch().set_autorelease(-1),
    ):
        try:
            build()
            raise AssertionError("should raise")
        except MakcuProtocolError:
            pass
    assert t.sent == []
    n = (
        m.batch()
        .move_now(1, 1)
        .moveto(100, 100)
        .silent(100, 100)
        .click(Button.RIGHT, 2, 50)
        .set_move_mask(1, 0, 0, 0)
        .set_wheel_mask(0, 1)
        .set_button_mask(Button.LEFT, True)
        .set_autorelease(1000)
        .heartbeat()
        .execute()
    )
    assert n == 9
    assert len(t.sent) == 1


def test_testbuild_features_with_mock():
    t = MockTransport()
    m = Mouse(t)
    assert m.api_version() == 1
    assert m.autorelease() == 0
    m.set_autorelease(5000)
    m.heartbeat()
    assert "km.autorelease(5000)" in t.sent
    assert "km.heartbeat()" in t.sent


def test_button_stream_with_mock():
    from makcu_engine.errors import MakcuTimeoutError

    t = MockTransport()
    m = Mouse(t)
    m.start_button_stream(1)
    t.push_stream_frame(b"km.\x01\r\n")
    assert m.poll_button_event() == 0x01
    m.stop_button_stream()
    assert "km.buttons(1)" in t.sent
    assert "km.buttons(0)" in t.sent
    try:
        m.poll_button_event()
        raise AssertionError("should raise")
    except MakcuTimeoutError:
        pass
