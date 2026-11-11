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
- [ ] README drift : réécrire listes API `Mouse` (`init/press/release/double_click`, observers, bezier/human/drag, `CLICK_PROFILES`) et `Transport` (callbacks, stats/ring, `no_wait`, profiler, `fire_and_forget`), layout (`test_{lifecycle,observe,humanize,transport_obs,remainder}.py`, `examples/{udp_relay,macros}.py`), features (humanisation, observabilité, géométrie), nombres (Python 95+8 HW, Rust 40), table de portage (`Duration`, `Vec`/array, arrondi half-away vs banker, `NoWaitGuard`, `TransportStats`, `BatchBuilder::move`→`move_rel`, `connect` idempotent, `wait_idle` poll, `Mock::is_connected` champ vs méthode). Acceptation : chaque `def` public cité ou marqué interne.
- [ ] `button_stream_state()` faux pour modes 2/3 (`mouse.py:332`, `== "1"`) : `!= "0"` ou `self._stream_mode`. Test mock `km.buttons:2/3`.
- [ ] `BatchBuilder` : `set_echo` ? / verdict `set_screen` (`confirm=True` sans équivalent batch) / `move_bezier` (trame 5-arg batchable) / alias `press/release` — implémenter à validation identique ou resserrer docstring `batch.py:7` + verdict.
- [ ] Dérive signatures mock/réel : kwargs `retries`/`error_window_s` acceptés partout, combler `abc.Transport` (`connect/close/reconnect/ensure_connected/callbacks/stats/no_wait`), `_note_tx` mock avec fallback `repr` (binaire). Test mypy sur les deux transports + `send_raw` binaire.
- [ ] Port Rust du profiler (zéro hit `profil*` dans `rust/`) : même sémantique opt-in + test (`MockTransport` + `MakcuTransport`).
- [ ] `move_smooth` Rust : division tronquée vs `round()` Python sur deltas négatifs (`-3/2` : `-2` vs `-1`, pas juste ±1) — arrondi documenté + test négatif.
- [ ] Mock Rust : bras `side1/side2` manquants (README "same canned replies" faux) — test ligne-à-ligne vs table Python.
- [ ] `cargo doc -D warnings` rouge (`types.rs:204,295` liens `MakcuError`, `mock.rs` liens redondants + doublons § P2) — réparer, ajouter au checklist CI.
- [ ] Tout-échec → `MakcuProtocolError` : `wait_idle(poll_s<0)` (`ValueError` stdlib), `drag_abs(duration=1e9)` (segments géants), `Monitor(scale=0)` (`ZeroDivisionError`) — cap ou raise + tests.
- [ ] Ownership tri-state : vérifier `ButtonOwnership` (NONE/PHYSICAL/INJECTED/BOTH) suffit — sinon query dédiée ; doc.
- [ ] `click(hold_ms=0)` : vérifier passthrough random-firmware 35–75 ms (spec binaire CLICK) ; doc + test, sinon verdict.

### P1 — utile
- [ ] Trous coverage listés : `batch` click+hold/`set_lock`, `events` stop-timeout, mock min/max, `mouse` step_s<0/mode-3 guard, `protocol` non-ascii/empty/`h`/CRLF, timeouts stream, jitter/validation `human_like`, `drag_abs` finally, `init()` partiel, `double_click` SIDE, `get_recent_lines(n>len)` — un test nommé par ligne ou `coverage: exclude` justifié.
- [ ] `tests/test_transport_unit.py` avec `FakeSerial` in-memory : query/strip, fenêtre ERR (dont `error_window_s=0`), `no_wait`, stats/ring/profile, `reconnect` — sans hardware.
- [ ] `ButtonMonitor` : `_run` survit à une frame malformée (`parse_button_event` lève → catch+compte+continue), `drain()` sous lock, `stop()` + alive-check, accesseur santé. Test frame poubelle puis re-`start()`.
- [ ] Convention messages d'erreur unifiée (`MakcuProtocolError("<param>…")`) + note `protocol.md` + tests de préfixes, les deux langages (`check_*` style drift Rust inclus).
- [ ] `click_human_like(rng=None)` injectable, défaut global (parité sampler Rust) ; tests existants inchangés.
- [ ] Petits manques : alias `scroll`, `udp_relay` 5 boutons, recettes `set_lock`/exemple doc, doc `is_locked` vs `is_pressed`, alias `enable_button_monitoring` — implémenter + test ou rejeter explicitement.
- [ ] `docs/protocol.md` : sous-sections `no_wait`, stats/ring/profiler, trame bezier 5-arg, `press/release/double_click/init`, coûts (5/14 queries), règle mode-3/loop + liens `makcu.com/mcp` et `/api/versions` comme source "depuis quel firmware".
- [ ] Knobs transport Rust : `ensure_connected` au minimum, `retries`/`error_window`/`candidates` portés ou documentés-intentionnels.
- [ ] `wait_idle` Rust : param `poll` + wrapper `wait_idle(timeout)` ; test du poll custom.
- [ ] Exemples Rust : `batch` + stream + stats sans click/scroll (ex. `batch_monitor.rs` HW-gated).
- [ ] `api_version()` comme capability gate (au lieu de tolérance de réponse), les deux stacks — S.
- [ ] `move_abs` portable bouclé sur `getpos()` (sans ctypes, clean-room, mêmes maths) — M.
- [ ] `move_pattern` waypoints + `click_sequence` (idée MIT, pas de copie) — M.
- [ ] Spike planificateur SigmaDrift (MIT : Fitts + sigma-lognormal, replay deltas) avec caveat détectabilité — L.
- [ ] Comparaison reconnect/backoff K4HVH (writer coalescent + monitor, `Send+Sync`) — n'adopter que mesuré — S.
- [ ] `km.catch_m*` : probe HW de capacité d'abord, pas d'API avant — S/M (idée MIT, V3 exclu sauf preuve).
- [ ] `button_up_force` : vérifier `up(silent_release)` couvre le stuck, sinon ajouter — S.

### P2 — hygiène / dette
- [ ] `ruff format --check` en CI ou décision "non enforced" (10 fichiers dérivent ; asymétrie avec `cargo fmt` enforced).
- [ ] `mypy tests examples` ou typer `parse_datagram`/`apply_action` (union `Action`) — `udp_relay` force des `type: ignore`.
- [ ] `macros.py` : `to_rel_deltas([])` → `ValueError`, validation coords au load, `scale_xy` rename (shadow core `scale_coordinates`), type `Event`.
- [ ] `__init__.py` : exporter `CLICK_PROFILES`, `CoordSpace` (+ test de surface packaging).
- [ ] Rust : `MakcuError: PartialEq/Eq/Clone`, jitter-0 `sent()` pin, `list_candidates` tier desc-fallback, note `fire_and_forget` Tx, organisation tests inline vs `tests/` (décider + noter).
- [ ] `move_now` sémantique V4.097+ (next free USB report) dans `protocol.md` ; re-mesure timings polling 1 ms avec § Hardware.

## Release — séquence exacte (audit 2026-11-11, `release.yml` tag `v*`)
- [ ] Bump (re-push `0.5.1`/`0.4.1` serait rejeté doublon) : `pyproject.toml` → `0.6.0`, `__init__.__version__` → `0.6.0`, `rust/Cargo.toml` workspace → `0.5.0` + pin `workspace.dependencies makcu_core` (`0.4.0` stale) → `0.5.0`, retitler CHANGELOG Unreleased → `## 0.6.0 (Python) / 0.5.0 (Rust) — <date>`.
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
Ajouts 2026-11-11 : WindMouse (GPL-3.0-only, incompatible MIT/Apache + détecté 97–100 % @ ≤1 % FPR — cside 2026) ; `mak-suite`/MAKXD (autre famille de device) ; async API ; auto-reconnect-dans-query (manuel `reconnect/ensure_connected` suffit).
