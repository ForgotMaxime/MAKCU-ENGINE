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

## [P0] 0.6.0 — Parité V4 / ergonomie (sans élargir le scope)
- [ ] `init()` + release-all sur connect/disconnect anti-stuck (`mouse.py`, `transport.py`) — acceptation : `test_mouse_mock` + HW `version/device` inchangés après `init`.
- [ ] Alias `press/release` fins sur `down/up` + `double_click(button, gap_ms)` (`mouse.py`) — acceptation : parité `makcu-py-lib`, zéro duplication de validation (`protocol.check_*`).
- [ ] Parité `BatchBuilder.up(silent_release=…)` avec `Mouse.up` (`batch.py`) — acceptation : test mock du flag `2`.
- [ ] `get_all_lock_states() + is_locked()` avec cache court (`mouse.py`, cf. `makcu-py-lib`) — acceptation : 1 query par appel groupé, pas N.
- [ ] `get_button_states()/get_button_mask()/is_pressed()` helpers sur `Mouse` ou `ButtonMonitor` (`mouse.py`/`events.py`) — acceptation : lecture sans effet de bord, testée sur mock.
- [ ] `on_connection_change(cb)` + `remove_connection_callback` (`transport.py`, cf. `makcu-py-lib`, crate `makcu`) — acceptation : callback sur `connect/disconnect/reconnect`, exemple monitor.
- [ ] Trancher `interpolation 0x1F` : pas de nom `km.*` texte connu → soit `send_raw` documenté, soit marqué non supporté ASCII (`docs/protocol.md`).
- [ ] Trancher stream `wheel-telemetry 0x20` + snapshots `0x55/0x56` : binaire → a priori non-goals, à documenter explicitement (`docs/protocol.md`).

## [P1] 0.7.0 — Humanisation `extras` (mêmes maths Python/Rust)
- [ ] `move_bezier(dx, dy, segments, ctrl_x, ctrl_y)` côté firmware (`km.move(x,y,seg,cx,cy)`, cf. `pymakcu`/`makcu-py-lib`) — acceptation : mock + HW, fallback `ctrl=dx//2, dy//2`.
- [ ] `click_human_like(button, count, profile, jitter)` : profils `normal/fast/slow/variable/gaming` + jitter, down 40-120 ms log-normal, 2 % outliers, double-click 80-180 ms (`mouse.py`, valeurs `makcu-py-lib`) — acceptation : test timings mockés, pas de `sleep` réel en unit.
- [ ] `drag_abs(start, end, button, duration)` canonique : move + press + `segments=max(10, duration*30)` + release + settles 20 ms (`mouse.py`, cf. `drag` existant relatif) — acceptation : exemple `smooth.py` étendu, retour home net-zéro HW.
- [ ] Mapping DPI/sens + `coord_space: virtual|monitor_local` explicite (`types.py`, `mouse.py`) — acceptation : jamais de 96 DPI implicite, doc + test de conversion.
- [ ] Macros JSON `{version, screenLayout, events}` : `dedupe_moves/scale_coordinates/adjust_delays(0.25-4x)`, replay en deltas rel (`examples/` d'abord, core ensuite si usage réel).

## [P1] 0.8.0 — Robustesse / observabilité
- [ ] Compteurs `tx_bytes/cmds_sent/reconnects/avg_rtt_ms/last_error` + ring buffer `get_recent_lines()` + `cmd_id` par move (`transport.py`).
- [ ] Profiler par commande, coût nul quand off (`protocol.py`, cf. crate `makcu` `profile`, `makcu-cpp PerformanceProfiler`).
- [ ] `FireAndForget` context manager Python (`ff()` RAII, cf. crate `makcu`) — acceptation : queries wait toujours, setters no-wait dans le bloc.
- [ ] Multi-écran : `Monitor{x,y,w,h,scale}` → `virtual_bounds` (origine négative OK), `to_local/to_virtual/remap_point` (`types.py` + helpers).
- [ ] Exemple relai UDP→série `:5005` (`examples/`, cf. `MakcuRelay` MIT) — exemple seul, pas dans le core.

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
