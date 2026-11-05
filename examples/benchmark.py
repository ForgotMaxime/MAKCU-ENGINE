"""Benchmark: query round-trip, fire-and-forget burst, coalesced batch.

No clicks, no scroll drift: moves are km.move(0,0) (full frame, zero delta)
or net-zero pairs.
"""

import time

from makcu_engine import MakcuTransport, Mouse

N = 20

with MakcuTransport.auto() as t:
    m = Mouse(t)

    t0 = time.perf_counter()
    for _ in range(N):
        m.echo()
    dt = (time.perf_counter() - t0) / N * 1000
    print(f"query round-trip (km.echo x{N}): {dt:.2f} ms avg")

    t0 = time.perf_counter()
    for _ in range(100):
        m.move_ff(0, 0)
    dt = (time.perf_counter() - t0) * 1000
    print(f"100x fire-and-forget km.move(0,0): {dt:.1f} ms total")

    t0 = time.perf_counter()
    for _ in range(N):
        m.batch().move(0, 0).move(0, 0).wheel(0).execute()
    dt = (time.perf_counter() - t0) / N * 1000
    print(f"batch x3 (x{N}): {dt:.2f} ms avg (1 write + 1 ERR-window)")
