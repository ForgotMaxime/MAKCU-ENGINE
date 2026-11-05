"""Smoke check: auto-detect, queries, net-zero micro-move. No clicks."""

from makcu_engine import MakcuTransport, Mouse
from makcu_engine.types import Button

with MakcuTransport.auto() as t:
    print(f"MAKCU on {t.port} @ {t.baudrate}")
    m = Mouse(t)
    print("version:", m.version())
    print("device:", m.device())
    print("screen:", m.screen(), "pos:", m.getpos(), "moving:", m.moving())
    print("left:", m.button_state(Button.LEFT))
    p0 = m.getpos()
    m.move(2, 0)
    m.move(-2, 0)
    print(f"move OK ({p0} -> {m.getpos()})")
