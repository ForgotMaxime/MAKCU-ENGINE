# Changelog

## 0.5.1 (Python) / 0.4.1 (Rust) — 2026-11-05

Second external audit, all points verified against code and fixed:
- Release: the crates.io wait loop now requires the exact published
  version (`makcu_core = "x.y.z"`), not just the crate name.
- Dead event reader fails fast with `MakcuConnectionError` (both stacks)
  instead of silent timeouts, so `ensure_connected()` can reconnect.
  Covered by a white-box reader-death test (Python) and the HW loop test.
- Rust `request()` no longer clears the input buffer while the loop owns
  the stream (was dropping unread OS-buffered frames).
- `MakcuTransport` is no longer `Clone` (Rust): `Drop` stops a running
  loop, which would have disturbed other owners of a shared port.
- `ButtonMonitor.start()` on a running monitor is an explicit error in
  both languages; `stop()` is idempotent; restart allocates a fresh
  thread (Python) instead of reusing a dead one.
- Callbacks are invoked after releasing locks (snapshot-then-dispatch),
  so a callback may register more callbacks or pump again.
- CI now runs Python 3.10 + 3.12; `tomllib` import has a stdlib-free
  fallback so the 3.10 job exercises the packaging test honestly.
- HW echo test restores echo-off via RAII guard even on failure.
- README: removed the duplicated Transport paragraph.
- Licensing groundwork: dual `MIT OR Apache-2.0` (`LICENSE-MIT`,
  `LICENSE-APACHE`), SPDX expression + author + repository wired into
  `pyproject.toml` and both crates (workspace inheritance); sole author,
  no third-party code copied, no secrets; only MPL-2.0 `serialport`
  dependency (weak file-level copyleft, compatible as an unmodified dep).
- Full doc coverage both stacks (`ruff D100-D103`, rustdoc `missing_docs`
  deny, `cargo doc -D warnings` in CI); `docs/protocol.md`; advanced
  examples (`smooth.py`, `monitor.py`) verified on hardware.
- Release pipeline hardened: `workspace.dependencies` (path-only dep broke
  `cargo publish`), PyPI long-description wired, `twine check` green,
  `cargo publish --dry-run` green for both crates, CI coverage gate 85 %
  (measured 96 %).

Completeness pass against official docs, the reference `makcu_api` and
hardware probes:
- Echo-aware replies: `strip_prompt` returns the last line, so queries
  parse identically with echo on or off (hardware-verified both ways).
- Byte-precise demux: `split_frames` gives stream frames priority only
  when shaped like one; echo lines stay glued to their reply. Interleaved
  frame+reply now splits correctly (was: swallowed into the reply).
- `button_stream_state()` (query `km.buttons()` → `"1"`), hardware-verified.
- Side buttons use the `ms1`/`ms2` wire names (like lock targets and the
  reference lib); both names accepted by the firmware, verified on device.
- `move_smooth` / `drag` scripted motion (firmware still interpolates
  each step), `wait_for_device` / `detect_all` discovery helpers.
- Packaging hardened: workspace-dependency for the serial crate (path-only
  broke `cargo publish`), PyPI long-description wired, `twine check` +
  both `cargo publish --dry-run` green locally; release workflow publishes
  core first then serial after index propagation.
- Full docstrings both stacks (`ruff D100-D103`, rustdoc `missing-docs`
  deny), 85% core-coverage gate green at 96%, advanced examples
  (`smooth.py`, `monitor.py`).

## 0.4.0 (Python) / 0.3.0 (Rust) — 2026-11-05

- Full-duplex event loop: background reader demultiplexes prompt replies
  vs stream lines (`split_frames`); queries keep working while a button
  stream is active (`start/stop_event_loop`, `poll_event`). Modes 1/2 only;
  bare mode 3 stays on the legacy poll path. Deadlock found & fixed during
  HW validation (reader must not hold the port lock across reads);
  `Drop` stops the loop. Hardware-verified incl. ERR-window timing.
- `ButtonMonitor`: pump thread, change callbacks, drain queue.
- Hot-plug: `wait_for_device()`, `detect_all()` for multi-device setups.
- `wait_idle(timeout)`: block until injected movement drains.
- `set_screen()` requires `confirm=True` (moves live tracking origin).
- Docs: `docs/protocol.md`, rustdoc `missing_docs` enforced, pydocstyle
  (`D100-D103`) enforced, `cargo doc -D warnings` in CI.
- CI: core coverage gate 85% (currently 96%), release workflow
  (tag-triggered; PyPI trusted publishing, crates.io dry-run).

Top-3 demandés, vérifiés hardware des deux côtés :
- Erreurs contextuelles : `set()` préfixe le nom de commande, `BatchBuilder.execute()`
  liste les commandes du batch en cas de rejet.
- `wait_idle(timeout)` : attend la fin des mouvements injectés (`km.moving` poll).
- Stream mode 3 : `poll_button_event()` lit le bare byte via `read_exact`
  (mode suivi dans `Mouse`, erreur explicite si poll sans `start`).

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
