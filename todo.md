# TODO

État : `0.5.1` Python / `0.4.1` Rust + vague Unreleased mergée (95 tests unit Python, 40 tests Rust, 7+8 HW-gated). Gate coverage core 85 % (full-tree ~76 %, transport série HW-only par design).
Dossier `inspiration/` : 11 clones lecture seule, gitignorés sauf `README.md` (licences mixtes dont GPL — idée seule, jamais de copie).

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

## Vague suivante — audits du 2026-11-11 (4 subagents : Python, Rust, web, release)
Firmware officiel toujours V4.108 : aucune nouvelle commande souris ASCII depuis V4.107, pas de course parité. Ordre suggéré : bump release → P0 → P1 → P2.

### P0 — correctness / bloque la release
- [x] README drift : listes API `Mouse`/`Transport`, layout, features, nombres, table de portage — mergé (`feat/docs-refresh` + retouche ligne profiler-Rust après `feat/rust-p0-fixes`).
- [x] `button_stream_state()` modes 2/3 (`!= "0"`) — mergé (`feat/py-p0-fixes`, test mock 0/1/2/3).
- [x] `BatchBuilder` : `set_echo`, `set_screen(confirm=…)` miroir Mouse, `move_bezier`, alias `press/release`, `scroll` ; docstring réécrite — mergé.
- [x] Signatures mock/réel unifiées (kwargs `retries`/`error_window_s`), `abc.Transport` étendu, `_note_tx` fallback `repr` — mergé + test mypy deux transports.
- [x] Port Rust du profiler (opt-in, sémantique snapshot, internals `Duration`) + test — mergé (`feat/rust-p0-fixes`).
- [x] `move_smooth` Rust : `round_ties_even` documenté + test négatif (`-3/-5`) — mergé.
- [x] Mock Rust : bras `side1/side2` + test table ligne-à-ligne vs Python — mergé.
- [x] `cargo doc -D warnings` vert (liens + déduplication doc) — mergé ; gate qualité de nouveau satisfiable.
- [x] Tout-échec → `MakcuProtocolError` : `wait_idle(poll_s<0)`, `drag_abs` cap 60 s, `Monitor(scale<=0)`, `scale_coordinates` — mergé + tests.
- [x] Ownership tri-state : déjà exposé (0/1/2/3), doc NONE/PHYSICAL/INJECTED/BOTH + test 4 états — pas de nouvelle query.
- [x] `click(hold_ms=0)` : passthrough random-firmware 35–75 ms (`km.click(b,count,0)`), `protocol.check_click` inchangé — mergé + doc + test.

### P1 — utile
- [x] Trous coverage listés — `tests/test_coverage_gaps.py` (19 tests nommés) + `test_transport_unit.py` (8 tests, serial mocké) — mergé (`feat/py-p1-tests`).
- [x] `tests/test_transport_unit.py` avec serial mocké in-memory : query/strip, fenêtre ERR (dont `error_window_s=0`), `no_wait`, stats/ring/profile, `reconnect` — sans hardware — mergé.
- [x] `ButtonMonitor` : `_run` survit aux frames malformées (compte + continue), `drain()` sous lock, `stop()` + alive-check, accesseurs `is_running`/`pump_errors` — mergé.
- [ ] Convention messages unifiée : appliquée aux fichiers touchés par py-p0 (`MakcuProtocolError("<param> …")`) — reste balayage `transport/types/macros` + `check_*` Rust + note `protocol.md` + tests de préfixes.
- [x] `click_human_like(rng=None)` injectable, défaut global — mergé (`feat/py-p0-fixes`, parité sampler Rust).
- [x] Petits manques : alias `scroll`, `udp_relay` 5 boutons, doc `is_locked` vs `is_pressed`, alias `enable_button_monitoring`, exports `__init__` — mergé. Reste si demande : conveniences `lock_left/…`.
- [x] `docs/protocol.md` : sous-sections `no_wait`, stats/ring/profiler, trame bezier 5-arg, `press/release/double_click/init`, coûts (5/14 queries), règle mode-3/loop + liens `makcu.com/mcp` et `/api/versions` — mergé (`feat/docs-refresh`).
- [x] Knobs transport Rust : `ensure_connected` implémenté (hook `check_connected`) ; `retries`/`error_window`/`candidates` documentés-intentionnels — mergé (`feat/rust-p1b`).
- [x] `wait_idle` Rust : `wait_idle_with_poll(timeout, poll)` + wrapper 20 ms ; poll négatif irreprésentable (`Duration`) — mergé + test.
- [ ] Exemples Rust : `batch` + stream + stats sans click/scroll (ex. `batch_monitor.rs` HW-gated).
- [x] `requires_api(min)` Rust — mergé ; pendant Python en cours (`feat/py-p1b`), parité à confirmer au merge.
- [ ] `move_abs` portable bouclé sur `getpos()` (sans ctypes, clean-room, mêmes maths) — M.
- [ ] `move_pattern` waypoints + `click_sequence` (idée MIT, pas de copie) — M.
- [ ] Spike `windmouse-rs` 0.1.1 (MIT, idée seule — pas de dép, core zéro-dép) : évaluer l'algo vent/gravité (param G/W/M/D, `max_step`, `target_area`) comme référence clean-room d'un planificateur `move_wind`, mêmes maths Python+Rust ; avec le caveat détectabilité (variation naturelle, pas d'indétectabilité — cside 2026). — S/M.
- [ ] Spike planificateur SigmaDrift (MIT : Fitts + sigma-lognormal, replay deltas) avec caveat détectabilité — L.
- [ ] Comparaison reconnect/backoff K4HVH (writer coalescent + monitor, `Send+Sync`) — n'adopter que mesuré — S.
- [ ] `km.catch_m*` : probe HW de capacité d'abord, pas d'API avant — S/M (idée MIT, V3 exclu sauf preuve).
- [x] `button_up_force` : pas d'ajout — `up(b, true)` (trame `2`) est le seul chemin force firmware, documenté (Python à confirmer).

### P2 — hygiène / dette
- [x] `ruff format --check` en CI — appliqué : gate ajoutée (`ci.yml`), 9 fichiers reformatés (`104f4d0`). Dérive future bloquée comme `cargo fmt`.
- [ ] `mypy tests examples` ou typer `parse_datagram`/`apply_action` (union `Action`) — `udp_relay` force des `type: ignore`.
- [x] `macros.py` edges : `to_rel_deltas([])` → `ValueError`, validation coords au load, `scale_xy` rename, type `Event` — mergé (+ tests).
- [x] `__init__.py` : `CLICK_PROFILES`, `CoordSpace` exportés — mergé (test de surface : à ajouter, voir P2 `mypy`/packaging si besoin).
- [x] Rust : `MakcuError: PartialEq/Eq/Clone`, jitter-0 `sent()` pin, `list_candidates` tier documenté-intentionnel, note `fire_and_forget` Tx, organisation tests inline — tous mergés.
- [x] `move_now` V4.097+ dans `protocol.md` — mergé (`feat/docs-refresh`) ; re-mesure timings avec § Hardware.

## Release — séquence exacte (audit 2026-11-11, `release.yml` tag `v*`)
- [x] Bump fait (`151bd62`, sans tag) : `0.6.0` / `0.5.0`, pin `makcu_core` corrigé, CHANGELOG retitré + section Unreleased rouverte. Reste : tag `v0.6.0` + push (déclenche la publish — confirmation requise).
- [ ] Valider en local : `pytest -k "not hardware"`, `ruff check src tests examples`, `mypy src`, `cargo test --workspace`, `clippy -D warnings`, `fmt --check`, `python -m build` + `twine check dist/*`, `cargo publish -p makcu_core --dry-run` (serial échoue en dry-run local par design — attend l'index).
- [ ] Commit bump, `git tag v0.6.0` (un seul tag, convention version Python), `git push origin main v0.6.0` → workflow `release` : PyPI trusted-publish + core→wait-loop→serial.
- [ ] Vérifier post-release : `pip index versions makcu-engine`, `cargo search makcu_core`, page crates.io, notes GitHub depuis CHANGELOG. Hygiène : Security tab, `pubtime`, bannières.
- [ ] Câbler trusted publishing pour de vrai : PyPI OIDC (`pypa/gh-action-pypi-publish`, env + reviewers, supprimer les tokens long-lived) ; crates.io (token manuel 1re fois puis `trusted-publishing-only`). Ajouter `twine check` pré-publish dans `release.yml`. P1 : `project.license` table dépréciée (cutoff setuptools 2027-02-18).

## Hardware à re-valider (`COM4 @ 4M`)
- [ ] `python -m pytest tests/ -v` (incl. 8 tests HW-gated).
- [ ] `cargo test --workspace` dans `rust/` (+ 7 tests HW ignorés en CI).
- [ ] `examples/benchmark.py`, `smooth.py`, `monitor.py` — re-mesurer baselines (polling 1 ms V4.108 + `move_now` next-report peuvent avoir bougé ~0.21 ms / 6.6 ms).
- [ ] Probe `km.catch_m*` : existe-t-il sur ce firmware ? (pas d'API avant preuve).
- [ ] Fenêtre ERR 80 ms : re-mesurer si firmware > V4.108 (`hw_setter_error_window_fast`).

## Qualité (avant chaque PR)
- [ ] `ruff + mypy --strict` et `clippy -D warnings + fmt --check` (et `cargo doc -D warnings` dès P0 réparé).
- [ ] Gate coverage core 85 % (full-tree ~76 %, transport série HW-only par design — qualifier, pas gonfler).
- [ ] `ruff format --check` : à trancher (voir P2), en attendant ne pas accroître la dérive.

## Dette / risques
- [ ] Cooldown CH343 Windows (retry backoff) — confirmer toujours nécessaire.
- [ ] Stream mode 3 (bare byte) legacy poll vs loop — la règle est en `protocol.md`, fermer la ligne ou acter le statu quo.
- [ ] `inspiration/makcu` vs `makcu_api` : vérifié identiques (diff récursif vide hors `.git`) — n'étudier qu'un seul des deux.
- [ ] `move_abs` Windows-only (`ctypes` `GetCursorPos`) : bug + hors scope → exclu (remplacé par item `move_abs` portable § P1).

## Non-goals explicites (ne pas ajouter)
Clavier (`down/up/string/remap`), controller, `flick`/`RAW_BATCH` (test-only), legacy V3 (`turbo/remap/swap_xy/catch_ml…` — `pymakcu.catch_button` est du V3), binaire `MAK_API`/`DE AD`, firmware custom (`medius`, `MAKCM`), flash, GUI.
Ajouts 2026-11-11 : paquet PyPI `windmouse` (GPL-3.0-only → incompatible MIT/Apache, ne pas porter ; de toute façon détecté 97–100 % @ ≤1 % FPR — cside 2026) — MAIS la réimplémentation Rust `windmouse-rs` 0.1.1 (`MostlyFinished`, crates.io) est MIT : utilisable comme référence d'idée seule (jamais de copie, jamais de dépendance — core zéro-dép), voir item P1 ; `mak-suite`/MAKXD (autre famille de device) ; async API ; auto-reconnect-dans-query (manuel `reconnect/ensure_connected` suffit).
