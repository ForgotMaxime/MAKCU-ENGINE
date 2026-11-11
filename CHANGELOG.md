# Changelog

## Unreleased

## 0.6.0 (Python) / 0.5.0 (Rust) — 2026-11-11

Quatre branches parallèles, mergées dans `main` après review, résolution
de conflits et re-validation (`pytest -k "not hardware"`, `ruff`, `mypy --strict`) :
- `feat/p0-lifecycle-aliases` : `Mouse.init()` (opt-in, jamais auto-appelé),
  alias `press`/`release`, `double_click`, `BatchBuilder.up(silent_release)`.
- `feat/p0-state-observe` : `get_all_lock_states/is_locked`,
  `get_button_states/mask/is_pressed`, callbacks `on_connection_change`
  (exceptions isolées + log) ; miroir mock (callbacks, `ms1`/`ms2`).
- `feat/p1-transport-docs` : compteurs `get_stats/reset_stats`, ring buffer
  `get_recent_lines` (fix `n<=0 → []` au merge), `no_wait()` imbricable,
  `Monitor`/`Bounds` + `virtual_bounds/to_local/to_virtual/remap_point`,
  verdicts `docs/protocol.md` (interpolation `0x1F`, snapshots `0x55/0x56`,
  wheel-telemetry : hors scope ASCII).
- `feat/p1-humanize` : `move_bezier` (firmware `km.move(x,y,seg,cx,cy)`),
  `click_human_like` (5 profils + jitter), `drag_abs` (`finally`-release),
  `CoordSpace` + `scale_coordinates` purs.
- Conflits de merge résolus : double `reconnect()` mock fusionné
  (close/reopen + compteur), blocs `mouse.py` complémentaires conservés.
  86 tests unitaires verts ; 4 tests HW pré-existants en échec environnemental
  (port série fantôme rejette `km.move`, identique sur `main` propre).
- `feat/py-remainder` : profiler opt-in (`enable/disable_profiling`,
  `get_profile` count/total/avg/min/max, coût-nul-quand-off, miroir mock),
  `examples/udp_relay.py` (UDP `:5005` → série, subset sûr), `examples/macros.py`
  (helpers `dedupe_moves/scale/adjust_delays`, replay en deltas rel).
- `feat/rust-mouse-parity` : parité `makcu_core` — `init/press/release`,
  `double_click(Duration)`, observers, `CLICK_PROFILES` + `move_bezier` /
  `click_human_like` (sampler `FnMut()->f64` injecté, zéro dép.) / `drag_abs`,
  `check_bezier`, `CoordSpace`/`Monitor`/`Bounds` + géométrie ; `BatchBuilder::up`
  breaking-change miroir Python. 32 tests verts.
- `feat/rust-transport-parity` : parité transport — `TransportStats`,
  ring `RECENT_CAPACITY=200`, `NoWaitGuard` RAII, callbacks `Arc<dyn Fn>`
  (panics non rattrapées, documenté), defaults de trait pour compatibilité ;
  miroir mock (8 tests) + impl `makcu_serial` (port sous `Mutex<Option<>>`,
  `reconnect` +300 ms). 40 tests verts au total workspace (7 HW ignorés).
- `feat/rust-p0-fixes` : profiler Rust (opt-in, internals `Duration`,
  jamais `send_raw`/`fire_and_forget`), `move_smooth` arrondi `round_ties_even`
  documenté + test négatif, mock `side1`/`side2`, `cargo doc -D warnings` vert
  (+ déduplication doc), `MakcuError: PartialEq/Eq/Clone`, jitter-0 pin,
  `list_candidates` 3-tiers documenté-intentionnel. 44 tests verts.
- `feat/py-p0-fixes` : `button_stream_state` modes 2/3, `BatchBuilder`
  (`set_echo/set_screen/move_bezier/press/release/scroll`, `click` 0-passthrough),
  signatures mock/réel + `abc.Transport` étendu, validation → `MakcuProtocolError`
  (`poll_s`, cap 60 s, `Monitor`), `hold_ms=0` random-firmware, ownership doc,
  `rng` injectable, alias `scroll`/`enable_button_monitoring`, exports
  `CLICK_PROFILES`/`CoordSpace`, relay 5 boutons. 22 tests (`test_p0.py`).

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
