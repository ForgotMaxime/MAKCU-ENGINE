# MAKCU V4 Text Protocol Contract

Discovered against hardware, encoded in both implementations
(`src/makcu_engine/`, `rust/makcu_core` + `rust/makcu_serial`).
Validated against V4 official on `COM4 @ 4000000`.

## Framing

Host sends `km.<name>(<csv>)\r`. Device replies `<result>\r\n>>> `.
The `km.` prefix on RX is optional; TX always keeps it for compatibility.

## Echo off (`km.echo(0)`)

Queries return `<result>\r\n>>> `. Mutations return **zero bytes** on success.
Never block on the prompt for setters: single 80 ms ERR window instead —
empty means Ok, a reply containing `ERR` means rejected
(`km.<cmd>\r\nERR\r\n>>> `, input echoed even with echo off).

## Discovery

Probe `4_000_000` then `115_200`. Send `km.version()\r`, accept if the reply
contains `km.MAKCU`. Consume the trailing prompt before streaming.
Port order: CH343 `VID_1A86/PID_55D3` first, then other USB serial, then rest.
`detect()` returns the first match; `detect_all()` lists every match
(multi-device); `wait_for_device()` blocks for hot-plug.

## Full-duplex streaming

`start_event_loop()` spawns a background reader that demultiplexes prompt
replies (pending request) from stream lines (event queue), so queries keep
working while a button stream is active. Framed modes 1/2 only:
`km.buttons(1)` streams `b"km." + mask + CRLF` per change.
Bare mode 3 has no terminator and stays on the legacy poll path
(`read_exact(1)`), which must not run concurrently with queries.

## Timing

Virtual screen defaults to `1920x1080`. `getpos` is an estimate from
accepted host reports. Prefer 4M for move+click combos: a 17-byte
`km.move(100,100)\r` takes ~42 µs at 4M vs ~1.48 ms at 115200,
over the 1 ms USB poll interval.

## Out-of-scope verdicts (ASCII-only driver)

- Interpolation `0x1F` has no `km.*` text name: the ASCII protocol
  exposes no interpolation command, so it is unsupported by this driver.
- Binary snapshots `0x55`/`0x56` are out of scope: this driver speaks
  ASCII `km.*` frames only, never raw binary opcodes.
- Wheel telemetry `0x20` is out of scope: no `km.*` text equivalent
  exists; wheel state travels via `km.wheel` injection only.
- Mode-3 legacy poll confirmed: bare single-byte frames carry no
  terminator, so mode 3 stays on the `read_exact(1)` poll path and
  must not run concurrently with queries (see full-duplex above).

## Button lifecycle & aliases

`press`/`release` are exact aliases of `down`/`up`: `press(b)` sends
`km.<method>(1)`; `release(b)` sends `km.<method>(0)`, or `(2)` with
`silent_release=True` (the release frame is skipped). `double_click(b)`
is two scheduled `km.click(<v>,1)` frames with one `gap_s` sleep between
them — no extra wire command. `init()` is best-effort only: `up` (`0`)
for all five buttons in canonical order, then `km.buttons(0)` if a
stream was started from this handle; every failure is swallowed, so
`init()` never raises.

## Bezier frame (`km.move` 5-arg)

`move_bezier(dx, dy, segments, cx, cy)` sends one frame
`km.move(dx,dy,segments,cx,cy)\r` — the curve runs firmware-side.
Validation: `segments` 1..=255, control point i16 on each axis, `dx/dy`
i16. An omitted control point defaults to `(dx//2, dy//2)` floor
division. There is no `BatchBuilder.move_bezier` in this snapshot;
the same frame can be sent raw via `send_raw(build_cmd(...))`.

## Per-call cost of observers

Observers are stateless — every call queries the device, nothing is cached:

| Helper | Wire cost |
|---|---|
| `button_state(b)` / `is_pressed(b)` | 1 query (`km.<method>`) |
| `get_button_states()` / `get_button_mask()` | 5 queries (one per canonical button) |
| `lock_state(t)` / `is_locked(t)` | 1 query (`km.lock_<target>`) |
| `get_all_lock_states()` | 14 queries (one per `LockTarget`, canonical order) |
| `button_stream_state()` | 1 query (`km.buttons`; true iff the reply is `1`) |

## `no_wait` semantics

Inside `with t.no_wait():` (Python) — or a `t.no_wait()` guard /
`with_no_wait(f)` scope (Rust) — `set()` degrades to `fire_and_forget`:
no ERR-window read. Scopes nest. Queries still block for their replies,
and `Batch.execute()` keeps its single ERR-window read (`send_raw` is
unaffected). Transmit counting (`get_stats`, ring) still records the frame.

## Observability: stats, ring, profiler

`get_stats()` snapshots `{tx_bytes, cmds_sent, reconnects, avg_rtt_ms,
last_error}` (Rust: the same keys on the `TransportStats` struct).
`reset_stats()` zeroes the counters and clears `last_error` but keeps the
ring. `get_recent_lines(n)` returns the last `n` `TX ...`/`RX ...` frames,
oldest-first, from a bounded 200-frame ring (`RECENT_CAPACITY` in Rust).
The per-command profiler is Python-only in this snapshot
(`enable/disable_profiling`, `profiling_enabled`, `get_profile`,
`reset_profile`; off by default behind a zero-cost single-`None` check):
`query`/`set` fold one duration per command
(`count/total_s/avg_ms/min_ms/max_ms`); `fire_and_forget`/`send_raw` never
record. Rust has counters, ring, `no_wait` and callbacks, but no profiler.

## `move_now` (V4.097+)

`km.move_now(dx,dy)` requests injection on the next free USB report
instead of the normal queue — same i16 validation as `move`, one frame,
same 80 ms ERR window. Prefer it for latency-sensitive nudges; prefer
`move` for ordered path drawing (`move_smooth`/bezier).

## Mode-3 / loop rule

Mode 3 frames are bare single bytes with no terminator: poll them only via
`read_exact(1)` on the legacy path (no queries in flight). Framed modes
1/2 (`km.` + mask + CRLF) work on both paths: with `start_event_loop()`
running, `poll_button_event` drains the demux queue and queries interleave
safely. With the loop running, requesting mode 3 is rejected — stop the
loop first and use the legacy poll path.

## Since-which-firmware

Command availability by firmware is tracked upstream, not here:

- <https://makcu.com/mcp>
- <https://makcu.com/api/versions>

Snapshot notes used in this tree: `move_now` V4.097+; `autorelease` /
`set_autorelease` / `heartbeat` V4.103+ (test builds, `ERR` on older
firmware); `api_version` V4.105 (reports `1`).
