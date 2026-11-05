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
