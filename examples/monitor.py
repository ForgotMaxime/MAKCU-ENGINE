"""Interactive button monitor: press physical mouse buttons, watch events.

Runs ~20 s, then exits. Queries keep working while the stream is live.
"""

from makcu_engine import ButtonMonitor, MakcuTransport, Mouse

with MakcuTransport.auto() as t:
    t.start_event_loop()
    try:
        m = Mouse(t)
        mon = ButtonMonitor(m)
        mon.on_change(lambda mask: print(f"buttons={mask:05b}"))
        mon.start(1)
        print("press physical mouse buttons, queries still work:")
        print("version:", m.version(), "| moving:", m.moving())
        import time

        time.sleep(20)
        mon.stop()
        print("caught:", mon.drain())
    finally:
        t.stop_event_loop()
