from makcu_engine.mock import MockTransport
from makcu_engine.mouse import Mouse
from makcu_engine.types import Button, LockTarget


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
    from makcu_engine.errors import MakcuProtocolError

    t = MockTransport()
    m = Mouse(t)
    try:
        m.poll_button_event()
        raise AssertionError("should raise")
    except MakcuProtocolError:
        pass
    m.start_button_stream(1)
    t.push_stream_frame(b"km.\x01\r\n")
    assert m.poll_button_event() == 0x01
    m.stop_button_stream()
    assert "km.buttons(1)" in t.sent
    assert "km.buttons(0)" in t.sent
    try:
        m.poll_button_event()
        raise AssertionError("should raise")
    except MakcuProtocolError:
        pass


def test_button_stream_mode3_with_mock():
    t = MockTransport()
    m = Mouse(t)
    m.start_button_stream(3)
    t.push_stream_frame(b"\x05")
    assert m.poll_button_event() == 0x05
    m.stop_button_stream()


def test_wait_idle_with_mock():
    t = MockTransport()
    m = Mouse(t)
    assert m.wait_idle(timeout_s=0.2) is True
    t2 = MockTransport(replies={"km.moving": "1"})
    m2 = Mouse(t2)
    assert m2.wait_idle(timeout_s=0.05, poll_s=0.01) is False


def test_batch_error_names_commands():
    from makcu_engine.errors import MakcuProtocolError

    t = MockTransport()
    t.fail_next_send = True
    m = Mouse(t)
    try:
        m.batch().move(1, 0).wheel(1).execute()
        raise AssertionError("should raise")
    except MakcuProtocolError as e:
        assert "km.move" in str(e) and "km.wheel" in str(e)


def test_set_error_names_command():
    from makcu_engine.errors import MakcuProtocolError

    t = MockTransport()
    m = Mouse(t)
    t.fail_next_send = True
    try:
        m.move(1, 0)
        raise AssertionError("should raise")
    except MakcuProtocolError as e:
        assert "km.move" in str(e)


def test_set_screen_requires_confirm():
    from makcu_engine.errors import MakcuProtocolError

    t = MockTransport()
    m = Mouse(t)
    try:
        m.set_screen(1920, 1080)
        raise AssertionError("should raise")
    except MakcuProtocolError:
        pass
    m.set_screen(1920, 1080, confirm=True)
    assert "km.screen(1920,1080)" in t.sent


def test_full_mouse_surface_with_mock():
    t = MockTransport()
    m = Mouse(t)
    assert m.version() == "km.MAKCU"
    assert m.device().route == "MK"
    assert m.echo() == 0
    m.set_echo(True)
    assert m.getpos().x == 100
    assert m.screen().width == 1920
    assert m.button_state(Button.RIGHT).value == 0
    assert m.lock_state(LockTarget.MX) is False
    m.move(1, 1)
    m.move_now(1, 1)
    m.moveto(10, 10)
    m.wheel(-1)
    m.silent(5, 5)
    m.set_move_mask(1, 0, 0, 0)
    m.set_wheel_mask(0, 1)
    m.set_button_mask(Button.MS1, True)
    m.set_lock(LockTarget.MX_POS, False)
    m.move_ff(0, 0)
    m.set_autorelease(100)
    m.heartbeat()
    assert m.api_version() == 1
    assert m.wait_idle(timeout_s=0.2) is True
    n = m.batch().moveto(1, 1).silent(2, 2).set_wheel_mask(1, 0).execute()
    assert n == 3


def test_mock_loop_flag_and_poll_event():
    t = MockTransport()
    assert t.event_loop_running() is False
    t.start_event_loop()
    assert t.event_loop_running() is True
    t.push_stream_frame(b"km.\x07\r\n")
    assert t.poll_event(0.1) == b"km.\x07\r\n"
    t.stop_event_loop()
    assert t.event_loop_running() is False


def test_monitor_start_stop_with_mock():
    from makcu_engine.events import ButtonMonitor

    t = MockTransport()
    m = Mouse(t)
    mon = ButtonMonitor(m)
    seen: list[int] = []
    mon.on_change(seen.append)
    mon.start(1)
    t.push_stream_frame(b"km.\x04\r\n")
    assert mon.pump_once(0.2) == 0x04
    assert mon.drain() == [0x04]
    mon.stop()
    assert "km.buttons(0)" in t.sent


def test_monitor_thread_start_stop():
    """Real pump thread runs and stops; no events without presses."""
    import time

    from makcu_engine.events import ButtonMonitor

    t = MockTransport()
    m = Mouse(t)
    mon = ButtonMonitor(m, poll_s=0.01)
    mon.on_change(lambda _mask: None)
    mon.start(1)
    time.sleep(0.1)
    mon.stop()
    assert "km.buttons(1)" in t.sent
    assert "km.buttons(0)" in t.sent


def test_button_stream_state_with_mock():
    t = MockTransport()
    m = Mouse(t)
    assert m.button_stream_state() is False
    t.replies["km.buttons"] = "1"
    assert m.button_stream_state() is True


def test_move_smooth_and_drag_with_mock():
    t = MockTransport()
    m = Mouse(t)
    m.move_smooth(10, 4, steps=5, step_s=0)
    moves = [c for c in t.sent if c.startswith("km.move(")]
    assert len(moves) == 5
    total = [0, 0]
    for c in moves:
        dx, dy = c[len("km.move(") : -1].split(",")
        total[0] += int(dx)
        total[1] += int(dy)
    assert total == [10, 4]
    t.sent.clear()
    m.drag(Button.SIDE1, 6, 0, steps=3, step_s=0)
    assert t.sent[0] == "km.ms1(1)"
    assert t.sent[-1] == "km.ms1(0)"
    assert len([c for c in t.sent if c.startswith("km.move(")]) == 3
    try:
        m.move_smooth(1, 1, steps=0)
        raise AssertionError("should raise")
    except ValueError:
        pass
