# Changelog

## 0.3.0 (Python) / 0.2.0 (Rust) — 2026-11-05

Validated against MAKCU V4 official on `COM4 @ 4000000` (`km.MAKCU`, `echo=0`).

Added:
- Auto-discovery: VID/PID-ordered port scan (`1A86:55D3` first) + baud probe.
  No port or baud hardcoded in the flow (`detect()` / `auto()` / `open_auto()`).
- `BatchBuilder`: coalesced single-write batches of setters (`batch().move()…​.execute()`).
- Test-build features with capability tolerance: `autorelease` / `heartbeat` / `api_version`
  (integration test asserts only when the firmware answers, skips otherwise).
- Button stream primitives: `start/stop_button_stream()` + `poll_button_event()`
  (single-threaded contract: no queries in flight while polling).
- `Button.MS1/MS2` aliases for `SIDE1/SIDE2` (Python `IntEnum`, Rust assoc consts).
- Resilience: query retry on timeout, `reconnect()` + `ensure_connected()`,
  open retry with backoff (Windows CH343 cooldown), idempotent context manager.
- Debug logging of raw frames (`logging.getLogger("makcu_engine")`, Python).
- `examples/benchmark.py`: round-trip / fire-and-forget / batch timings.
- CI: GitHub Actions (`pytest`, `cargo test`, `clippy -D warnings`, `fmt --check`, `ruff`, `mypy`).

Measured (COM4 @ 4M): query round-trip ~0.21 ms avg, 100× FF ~6.6 ms,
batch×3 ~94 ms (dominated by the 80 ms ERR window — 1 window per batch vs 1 per setter).

## 0.2.0 (Python) — 2026-11-05
- Generic discovery (`detect`/`auto`), `__enter__` idempotency fix.

## 0.1.0 — 2026-11-05
- Initial MVP: transport, mouse API, mock, parity Python/Rust, hardware smoke check.
