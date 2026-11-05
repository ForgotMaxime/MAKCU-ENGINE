# Changelog

## 0.3.1 (Python) / 0.2.2 (Rust) — 2026-11-05

Audit fixes (all six points verified against code, both stacks):
- Rust `send_raw`: single critical section (no lock drop between write and
  ERR-window read); `read_until` clamps the port timeout to 10 ms polls so a
  call can never exceed its deadline (previously one `read()` could block the
  full 800 ms port timeout). Hardware-timed: rejected setter < 500 ms.
- Shared validators (`protocol.check_*`, used by `Mouse` and `BatchBuilder`
  alike): `silent`/`moveto` coordinate ranges, screen size on parse, click
  args in batches — no more per-path divergence.
- Python parsers raise `MakcuProtocolError` (also a `ValueError` subclass for
  stdlib compat) instead of bare `ValueError`/`IndexError`.
- Batch parity: identical setter set in both languages
  (`move/move_now/moveto/silent/wheel/down/up/click/masks/lock/autorelease/heartbeat`).
- Versions: Rust workspace single source (`[workspace.package]` + `version.workspace`);
  Python `pyproject == __init__.__version__` enforced by `tests/test_packaging.py`.

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
