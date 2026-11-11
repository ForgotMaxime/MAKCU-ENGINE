# TODO

État : `0.5.1` Python / `0.4.1` Rust. Couverture core 96 % (gate 85 %).
Dossier `inspiration/` : 11 clones lecture seule, gitignorés sauf `README.md` (licences mixtes dont GPL — ne pas copier sans vérifier).

Légende : `[P0]` critique, `[P1]` utile, `[P2]` idée. Cible de fichier entre parenthèses.

## FAIT (0.5.1 — ne pas rouvrir)
- Mouvements : `move`, `move_now`, `moveto`, `silent`, `move_smooth` (linéaire host-side), `drag` (relatif), `move_ff` (`src/makcu_engine/mouse.py`).
- État : `version/device/echo/getpos/screen/set_screen(confirm=True)/moving/wait_idle/button_state/lock_state`.
- Boutons : `down/up` (+`silent_release`), `click(button, count 1..255, hold_ms)` complet.
- Masques/locks : `set_move_mask(4-dir)/set_wheel_mask(2-dir)/set_button_mask/set_lock`.
- Sécurité : `autorelease/heartbeat/api_version` (`src/makcu_engine/mouse.py`, `batch.py`).
- Batch : `BatchBuilder.execute()` une écriture + une fenêtre ERR (`src/makcu_engine/batch.py`).
- Stream : `start/stop/button_stream_state/poll_button_event` modes 1-3 + `ButtonMonitor` (`events.py`) + loop demux (`transport.py`).
- Discovery : `list_candidates/detect/detect_all/wait_for_device/auto/reconnect/ensure_connected` (`transport.py`).

## [P0] 0.6.0 — Parité V4 / ergonomie (sans élargir le scope) — PARTIE MERGÉE 2026-11-11
- [x] `init()` + release-all opt-in explicite, jamais auto-appelé (`mouse.py`) — mergé.
- [x] Alias `press/release` sur `down/up` + `double_click` (`mouse.py`) — mergé.
- [x] Parité `BatchBuilder.up(silent_release=…)` (`batch.py`) — mergé.
- [x] `get_all_lock_states() + is_locked()` stateless (`mouse.py`) — mergé.
- [x] `get_button_states()/get_button_mask()/is_pressed()` (`mouse.py`) — mergé.
- [x] `on_connection_change(cb)` + `remove_connection_callback` (`transport.py`, miroir mock) — mergé.
- [x] Trancher `interpolation 0x1F` : pas de nom `km.*` texte connu → soit `send_raw` documenté, soit marqué non supporté ASCII (`docs/protocol.md`) — mergé (verdict non supporté, `feat/p1-transport-docs`).
- [x] Trancher stream `wheel-telemetry 0x20` + snapshots `0x55/0x56` : binaire → a priori non-goals, à documenter explicitement (`docs/protocol.md`) — mergé (verdicts `feat/p1-transport-docs`).

## [P1] 0.7.0 — Humanisation `extras` (mêmes maths Python/Rust) — MERGÉ 2026-11-11
- [x] `move_bezier(dx, dy, segments, ctrl_x, ctrl_y)` firmware + `check_bezier` (`mouse.py`, `protocol.py`) — mergé.
- [x] `click_human_like(button, count, profile, jitter)` 5 profils + jitter (`mouse.py`, `CLICK_PROFILES`) — mergé.
- [x] `drag_abs(start, end, button, duration)` canonique (`mouse.py`, release via `finally`) — mergé.
- [x] Mapping DPI/sens : `CoordSpace` + `scale_coordinates()` purs (`types.py`, sans état global) — mergé.
- [x] Parité Rust `makcu_core` : `init/press/release/double_click(Duration)`, observers, `CLICK_PROFILES` + `move_bezier/click_human_like` (sampler injecté) `/drag_abs`, `check_bezier`, `CoordSpace/Monitor/Bounds` + géométrie — mergé (`feat/rust-mouse-parity`, 32 tests). Écarts doc : `Duration` vs float, `Vec`/array vs `dict`, arrondi half-away vs banker (±1 sur égalités).
- [ ] Macros JSON `{version, screenLayout, events}` : `dedupe_moves/scale_coordinates/adjust_delays(0.25-4x)`, replay en deltas rel — helpers + exemple mergés (`feat/py-remainder`) ; reste : intégration core si usage réel.

## [P1] 0.8.0 — Robustesse / observabilité — MERGÉ 2026-11-11
- [x] Compteurs `tx_bytes/cmds_sent/reconnects/avg_rtt_ms/last_error` + ring buffer `get_recent_lines()` (`transport.py`, miroir mock ; parité Rust : `TransportStats`, ring 200, `reconnect` compté) — mergé.
- [x] `no_wait()` imbricable Python + `NoWaitGuard` RAII Rust (`set()` → fire-and-forget, queries inchangées) — mergé.
- [x] Multi-écran : `Monitor`/`Bounds` + `virtual_bounds/to_local/to_virtual/remap_point` (Python + Rust) — mergé.
- [x] Profiler par commande, coût nul quand off (Python `enable/disable_profiling/get_profile`, miroir mock) — mergé.
- [x] Exemple relai UDP→série (`examples/udp_relay.py`, subset sûr) + macros JSON (`examples/macros.py`, plan par défaut) — mergés.

## Release
- [ ] Tag + publish PyPI (trusted publishing à tester pour de vrai, `twine check` vert).
- [ ] `cargo publish` `makcu_core` puis `makcu_serial` (dry-run vert).
- [ ] Vérifier SPDX `MIT OR Apache-2.0` résolu côté PyPI / crates.io après publish.

## Hardware à re-valider (`COM4 @ 4M`)
- [ ] `python -m pytest tests/ -v` (incl. 8 tests HW-gated).
- [ ] `cargo test --workspace` dans `rust/` (+ 7 tests HW ignorés en CI).
- [ ] `examples/benchmark.py`, `smooth.py`, `monitor.py` — confirmer timings (~0.21 ms query, ~6.6 ms 100× FF).

## Qualité (avant chaque PR)
- [ ] `ruff + mypy --strict` et `clippy -D warnings + fmt --check`, `cargo doc -D warnings`.
- [ ] Gate coverage core 85 % (mesuré 96 %).

## Dette / risques
- [ ] Fenêtre ERR 80 ms : re-mesurer si firmware > V4.108 (`hw_setter_error_window_fast`).
- [ ] Cooldown CH343 Windows (retry backoff) — confirmer toujours nécessaire.
- [ ] Stream mode 3 (bare byte) legacy poll vs loop — décider définitif + documenter.
- [ ] `inspiration/makcu` vs `makcu_api` : vérifié identiques (diff récursif vide hors `.git`) — n'étudier qu'un seul des deux.
- [ ] `move_abs` Windows-only (`ctypes` `GetCursorPos`) vu dans `makcu-py-lib` : bug (`debug=` forwardé à tort) + hors scope cross-platform → exclu sauf demande explicite.

## Non-goals explicites (ne pas ajouter)
Clavier (`down/up/string/remap`), controller, `flick`/`RAW_BATCH` (test-only), legacy V3 (`turbo/remap/swap_xy/catch_ml…` — `pymakcu.catch_button` est du V3), binaire `MAK_API`/`DE AD` (cf. `mak-suite`), firmware custom (`medius`, `MAKCM`), flash, GUI.
