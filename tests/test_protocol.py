from makcu_engine import protocol
from makcu_engine.types import Button


def test_build_cmd():
    assert protocol.build_cmd("km.move", 10, -3) == b"km.move(10,-3)\r"
    assert protocol.build_cmd("km.version") == b"km.version()\r"
    assert protocol.build_cmd("km.click", 1, 3) == b"km.click(1,3)\r"


def test_strip_prompt():
    assert protocol.strip_prompt(b"km.MAKCU\r\n>>> ") == "km.MAKCU"
    assert protocol.strip_prompt(b"0\r\n>>> ") == "0"
    assert protocol.strip_prompt(b"km.screen(1920,1080)\r\n>>> ") == "km.screen(1920,1080)"


def test_parse_device():
    d = protocol.parse_device("R:MK;M:16uf;K:8uf;C:0uf")
    assert (d.route, d.mouse_period, d.keyboard_period, d.controller_period) == ("MK", "16uf", "8uf", "0uf")


def test_parse_screen_getpos():
    assert protocol.parse_screen("km.screen(1920,1080)").width == 1920
    p = protocol.parse_getpos("km.getpos(123,456)")
    assert (p.x, p.y) == (123, 456)


def test_clamp_and_buttons():
    try:
        protocol.clamp_i16(40000)
        raise AssertionError("should raise")
    except ValueError:
        pass
    assert protocol.button_method(Button.LEFT) == "left"
    assert protocol.mask_method(Button.RIGHT) == "right_mask"
