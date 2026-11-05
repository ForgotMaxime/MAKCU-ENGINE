from makcu_engine import protocol
from makcu_engine.errors import MakcuProtocolError
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


def test_button_aliases():
    assert Button.MS1 is Button.SIDE1
    assert Button.MS2 is Button.SIDE2
    assert protocol.button_method(Button.MS1) == "side1"


def test_button_event():
    assert protocol.parse_button_event(b"km.\x03\r\n") == 0x03
    for bad in (b"garbage\r\n", b"km.\x03", b"", b"km.\x03\r\n>>> "):
        try:
            protocol.parse_button_event(bad)
            raise AssertionError(f"should raise for {bad!r}")
        except MakcuProtocolError:
            pass


def test_malformed_replies_raise_protocol_error():
    for bad in ("km.screen(1,2", "km.screen(a,b)", "nope", "km.getpos(1)"):
        for fn in (protocol.parse_screen, protocol.parse_getpos):
            try:
                fn(bad)
                raise AssertionError(f"should raise for {bad!r}")
            except MakcuProtocolError:
                pass
    try:
        protocol.strip_prompt(b"no prompt here")
        raise AssertionError("should raise")
    except MakcuProtocolError:
        pass
    try:
        protocol.parse_screen("km.screen(0,1080)")
        raise AssertionError("should raise")
    except MakcuProtocolError:
        pass


def test_shared_validators():
    for fn in (
        lambda: protocol.check_xy(-1, 0),
        lambda: protocol.check_xy(0, 32768),
        lambda: protocol.check_mask_bit(2),
        lambda: protocol.check_click(0, 1, None),
        lambda: protocol.check_click(1, 0, None),
        lambda: protocol.check_click(1, 1, 0),
        lambda: protocol.check_stream_mode(4),
        lambda: protocol.check_autorelease(60001),
    ):
        try:
            fn()
            raise AssertionError("should raise")
        except MakcuProtocolError:
            pass
