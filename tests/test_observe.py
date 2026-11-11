"""Stateless observers + connection callbacks — MockTransport only, no serial."""

from makcu_engine.mock import MockTransport
from makcu_engine.mouse import Mouse
from makcu_engine.types import Button, ButtonOwnership, LockTarget


def test_lock_states_dict_covers_all_targets():
    t = MockTransport(replies={"km.lock_ml": "1", "km.lock_mx+": "1"})
    m = Mouse(t)
    states = m.get_all_lock_states()
    assert set(states) == {tgt.value for tgt in LockTarget}
    assert states["ml"] is True
    assert states["mx+"] is True
    assert states["mr"] is False
    assert states["mw-"] is False
    # One query per target: stateless aggregate, no caching.
    assert len(t.sent) == len(LockTarget)


def test_is_locked_single_query():
    t = MockTransport(replies={"km.lock_my": "1"})
    m = Mouse(t)
    assert m.is_locked(LockTarget.MY) is True
    assert m.is_locked(LockTarget.MX) is False
    assert t.sent == ["km.lock_my()", "km.lock_mx()"]


def test_button_states_and_mask():
    t = MockTransport(
        replies={
            "km.left": "2",  # INJECTED -> bit 0
            "km.right": "1",  # PHYSICAL only -> no bit
            "km.middle": "3",  # BOTH -> bit 2
            "km.ms1": "0",  # NONE -> no bit
            "km.ms2": "2",  # INJECTED -> bit 4
        }
    )
    m = Mouse(t)
    states = m.get_button_states()
    assert states == {
        Button.LEFT: ButtonOwnership.INJECTED,
        Button.RIGHT: ButtonOwnership.PHYSICAL,
        Button.MIDDLE: ButtonOwnership.BOTH,
        Button.SIDE1: ButtonOwnership.NONE,
        Button.SIDE2: ButtonOwnership.INJECTED,
    }
    assert m.get_button_mask() == 0x01 | 0x04 | 0x10
    # Bit layout is 1 << (button.value - 1).
    assert m.get_button_mask() == (
        (1 << (Button.LEFT.value - 1))
        | (1 << (Button.MIDDLE.value - 1))
        | (1 << (Button.SIDE2.value - 1))
    )


def test_is_pressed_logic_incl_both():
    t = MockTransport(
        replies={
            "km.left": "0",
            "km.right": "1",  # physical-only: not an injected press
            "km.middle": "2",
            "km.ms1": "3",  # BOTH counts as pressed
            "km.ms2": "0",
        }
    )
    m = Mouse(t)
    assert m.is_pressed(Button.LEFT) is False
    assert m.is_pressed(Button.RIGHT) is False
    assert m.is_pressed(Button.MIDDLE) is True
    assert m.is_pressed(Button.SIDE1) is True
    assert m.is_pressed(Button.SIDE2) is False
    # Single query per call.
    assert t.sent.count("km.ms1()") == 1


def test_connection_callbacks_fired_on_connect_disconnect():
    t = MockTransport()
    seen: list[bool] = []
    t.on_connection_change(seen.append)
    assert t.is_connected is False
    t.connect()
    assert t.is_connected is True
    t.close()
    assert t.is_connected is False
    assert seen == [True, False]


def test_connection_callback_removal_works():
    t = MockTransport()
    seen: list[bool] = []
    cb = t.on_connection_change(seen.append)
    t.remove_connection_callback(cb)
    t.connect()
    t.close()
    assert seen == []
    # Removing twice / unknown callbacks is silent.
    t.remove_connection_callback(cb)
    t.remove_connection_callback(lambda _c: None)


def test_connection_callback_exception_isolated():
    t = MockTransport()
    seen: list[bool] = []

    def bad(_connected: bool) -> None:
        raise RuntimeError("boom")

    t.on_connection_change(bad)
    t.on_connection_change(seen.append)
    # A raising callback must not break connect/close or later callbacks.
    t.connect()
    t.close()
    assert seen == [True, False]
    assert t.is_connected is False


def test_connection_callback_may_register_during_dispatch():
    t = MockTransport()
    calls: list[bool] = []

    def first(_connected: bool) -> None:
        # Snapshot semantics: registered mid-dispatch, fires from next event.
        t.on_connection_change(calls.append)

    t.on_connection_change(first)
    t.connect()
    assert calls == []
    t.close()
    assert calls == [False]


def test_reconnect_emits_false_then_true():
    t = MockTransport()
    seen: list[bool] = []
    t.on_connection_change(seen.append)
    t.connect()
    t.reconnect()
    assert seen == [True, False, True]
