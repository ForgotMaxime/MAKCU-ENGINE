"""Smooth-move + drag demo. Net cursor displacement is zero (returns home)."""

import time

from makcu_engine import MakcuTransport, Mouse
from makcu_engine.types import Button

with MakcuTransport.auto() as t:
    m = Mouse(t)
    p0 = m.getpos()
    print(f"start: {p0}")
    m.move_smooth(120, 60, steps=12, step_s=0.01)
    m.wait_idle(timeout_s=2.0)
    print(f"moved: {m.getpos()}")
    time.sleep(0.2)
    m.move_smooth(-120, -60, steps=12, step_s=0.01)
    m.wait_idle(timeout_s=2.0)
    print(f"back home: {m.getpos()} (started {p0})")
    # Drag draws a 40px square with the left button held, then releases.
    m.drag(Button.LEFT, 40, 0, steps=5, step_s=0.01)
    m.drag(Button.LEFT, 0, 40, steps=5, step_s=0.01)
    m.drag(Button.LEFT, -40, 0, steps=5, step_s=0.01)
    m.drag(Button.LEFT, 0, -40, steps=5, step_s=0.01)
    print(f"square done: {m.getpos()}")
