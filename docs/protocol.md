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
