# MAKCU Engine — V4 Mouse-Only Driver

Python reference implementation with an exact Rust port. ASCII text protocol only (`km.*`, `\r`-terminated). No binary `MAK_API` framing.

## Overview

Small, dependency-light driver for MAKCU V4 over the management COM port (USB2/CH343). It owns the serial line exclusively, frames commands, parses the `>>> ` prompt, and exposes a validated mouse API. No keyboard, controller, or flashing logic.

Target firmware: **V4 official (validated against V4.108-class behavior)**. Baud negotiation order: `4_000_000` then `115_200`. No port or baud is hardcoded in the flow — both are detected at runtime (CH343 `VID_1A86/PID_55D3` preferred, then any serial port, each probed with `km.version()`).

## Features

- Transport: exclusive open, `8N1`, no flow control, baud probing (`4M` → `115200`)
- Framing: `km.name(a,b)\r` → reply `<payload>\r\n>>> `
- Query path blocks on prompt; mutation path is silent-aware (see contract)
- Mouse API: relative/absolute moves, buttons, wheel, click scheduler, masks, locks, virtual screen
- Abstractions: `Transport` trait/protocol for dependency injection
- Testing: pure unit tests + mock transport + hardware-gated integration (read-only + net-zero moves, no clicks)

Non-goals: binary protocol, keyboard/controller injection, device flashing, GUI.

## Repository Layout

```text
CB/
  src/makcu_engine/        # Python package (typed, src-layout)
    protocol.py            # pure framing/parsing — primary Rust-port surface
    types.py               # domain types, no I/O
    abc.py                 # Transport protocol (structural interface)
    transport.py           # serial Transport impl (thread-safe, exclusive)
    mouse.py               # Mouse<T: Transport> — validation + command mapping
    mock.py                # in-memory Transport for unit tests
    batch.py               # BatchBuilder — coalesced single-write setter batches
    config.py / errors.py
  tests/
    test_protocol.py       # pure, no hardware
    test_mouse_mock.py     # MockTransport, no hardware
    test_integration.py    # COM4-gated, safe probes only
  examples/check_mouse.py  # smoke check against real hardware
  rust/
    makcu_core/            # zero-dependency port: error/types/protocol/transport/mock/mouse
    makcu_serial/          # serial Transport impl (serialport 4) + hardware example
```

## Requirements

Hardware (validated):

| Signal | Value |
|---|---|
| Port | `COM4` — `USB-Enhanced-SERIAL CH343 (VID_1A86/PID_55D3)` |
| Mode | normal (passthrough `USB3 → USB1` active) |
| Baud | `4_000_000`, `8N1`, exclusive access |
| Firmware behavior | `km.version()` → `km.MAKCU`, `echo=0`, `device=R:MK;M:8uf;K:8uf;C:0uf` |

Software:

| Toolchain | Version |
|---|---|
| Python | `>=3.10`, `pyserial>=3.5`, `pytest` |
| Rust | `cargo/rustc 1.96`, `serialport 4` |

## Protocol Contract

Discovered against hardware, encoded in both implementations:

1. **Framing:** host sends `km.<name>(<csv>)\r`. Device replies `<result>\r\n>>> `. The `km.` prefix on RX is optional; TX always keeps it for compatibility.
2. **Echo off (`km.echo(0)`):** queries return `<result>\r\n>>> `. Mutations return **zero bytes** on success. Do not block on prompt for setters.
3. **Error signaling:** rejected commands return `ERR` (echoed input + `\r\nERR\r\n>>> ` even with echo off). Setters use an 80 ms error window: empty → `Ok`, `ERR` → `MakcuProtocolError`.
4. **Discovery:** probe `4_000_000` then `115_200`. Send `km.version()\r`, accept if reply contains `km.MAKCU`. Consume the trailing prompt before streaming.
5. **Timing:** virtual screen defaults to `1920x1080`. `getpos` is an estimate from accepted host reports. Prefer `4M` for move+click combos (17 B ≈ 42 µs at 4M vs 1.48 ms at 115200, over the 1 ms USB poll).

## Python Engine

Install (editable) and test:

```powershell
pip install -e .
python -m pytest tests/ -v
python examples/check_mouse.py
```

Minimal usage (port + baud auto-detected):

```python
from makcu_engine import MakcuTransport, Mouse
from makcu_engine.types import Button

with MakcuTransport.auto() as transport:
    print(f"using {transport.port} @ {transport.baudrate}")
    mouse = Mouse(transport)
    print(mouse.version(), mouse.device(), mouse.screen())
    mouse.move(2, 0)
    mouse.move(-2, 0)  # net-zero probe
    mouse.click(Button.LEFT, count=1)
```

Lower level, when you already know the endpoint:

```python
port, baud = MakcuTransport.detect()          # (port, baud) scan
candidates = list_candidates()                # ordered port names
with MakcuTransport(port, baud) as transport: ...
```

API surface (`Mouse`): `version/device/echo/set_echo/getpos/screen/moving/button_state/lock_state/move/move_now/moveto/set_screen/wheel/silent/down/up/click/set_move_mask/set_wheel_mask/set_button_mask/set_lock/move_ff/batch/autorelease/set_autorelease/heartbeat/api_version/start_button_stream/stop_button_stream/poll_button_event`.

Transport: `detect()` → `(port, baud)`, `auto()` (detect + connect), `open(port, baud)`, `probe(port)`, `reconnect()`, `ensure_connected()`, `send_raw()`, `read_line()`. Queries retry once on timeout; setters use a single 80 ms ERR window. Debug logging via `logging.getLogger("makcu_engine")`.

Batching (one write, one ERR window — same setter set and validation as `Mouse`):

```python
with MakcuTransport.auto() as t:
    n = (
        Mouse(t).batch()
        .move(2, 0).move(-2, 0).wheel(0)
        .moveto(100, 100).silent(100, 100)
        .click(Button.LEFT, 2, 50)
        .execute()
    )
```

Button stream (no queries in flight while polling):

```python
mouse.start_button_stream(1)
mask = mouse.poll_button_event(0.5)  # bitmask int
mouse.stop_button_stream()
```

Measured on `COM4 @ 4M` (`examples/benchmark.py`): query ~0.21 ms avg, 100×
fire-and-forget ~6.6 ms total, batch×3 ~94 ms (1 ERR window per batch vs 1 per setter).

Error model: `MakcuError` → `MakcuConnectionError | MakcuTimeoutError | MakcuProtocolError`. All parsers and argument validators raise `MakcuProtocolError` (also a `ValueError` subclass, so stdlib-style `except ValueError` keeps working).

Versions: Python declares once in `pyproject.toml`, asserted equal to `makcu_engine.__version__` by `tests/test_packaging.py`. Rust declares once in `rust/Cargo.toml` (`[workspace.package]`), members inherit via `version.workspace = true`.

## Rust Workspace

```powershell
cd rust
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
cargo run -p makcu_serial --example check_mouse
```

Minimal usage (port + baud auto-detected):

```rust
use makcu_serial::{MakcuTransport, Mouse};
use std::time::Duration;

let transport = MakcuTransport::open_auto(Duration::from_millis(800))?;
println!("using {} @ {}", transport.port_name(), transport.baud());
let mouse = Mouse::new(&transport);
mouse.move_rel(2, 0)?;
mouse.move_rel(-2, 0)?;
```

Lower level: `MakcuTransport::list_candidates()` (CH343 VID/PID first), `MakcuTransport::probe(port)` (baud scan), `MakcuTransport::open(port, baud, timeout)`.

Crates:

- `makcu_core` — zero dependencies. `Transport` trait (+ blanket `impl Transport for &T`), `BatchBuilder`, `MockTransport`, `Mouse<T: Transport>`, pure `protocol`/`types` with shared `check_*` validators. 11 unit tests (+ 3 hardware-gated, ignored in CI).
- `makcu_serial` — `Transport for MakcuTransport` over `serialport`. Interior mutability via `Mutex`, `read_until` with 10 ms polls under caller deadline (never exceeds the ERR window), single-critical-section `send_raw`, open retry with backoff.

## Hardware Validation

| Check | Python | Rust | Result (observed instance) |
|---|---|---|---|
| `detect()` (VID/PID-ordered scan + probe) | `test_integration` | `check_mouse` | `COM4 @ 4000000` |
| `version/device/echo/screen/getpos/moving` | pass | pass | `km.MAKCU`, `MK/8uf`, `1920x1080` |
| `move(+2,0)+move(-2,0)` net-zero | pass | pass | position unchanged |
| batch full-set + `autorelease` roundtrip | pass (unit + HW) | pass (unit + HW) | firmware answers `autorelease` |
| rejected-setter ERR window | unit | unit + HW-timed < 500 ms | `hw_setter_error_window_fast` (ignored) |
| Unit (framing, parsing, validation, mock flow, batch, stream) | 14 passed* | 11 passed | — |
| Lints | `ruff + mypy --strict` | `clippy -D warnings`, `fmt` | clean |

\* 19 with the 3 hardware-gated integration tests and 2 packaging tests.

Integration tests never click, never scroll, and never change persistent settings. Only `COM4` is touched; `COM3` (virtual, not ready) and `COM5/COM6` (ESP32-S3, `Unknown`) are left alone.

## Porting Notes

| Python | Rust | Note |
|---|---|---|
| `Mouse.move(dx, dy)` | `Mouse::move_rel(dx, dy)` | `move` is a keyword in Rust |
| `abc.Transport` (Protocol) | `makcu_core::Transport` (trait) | `query/set/fire_and_forget/send_raw/read_line` |
| `MockTransport` | `makcu_core::MockTransport` | same canned replies, `sent()` log, stream-frame queue |
| `MakcuProtocolError` (+ `ValueError`) | `MakcuError::{Protocol, InvalidArgument}` | validation/parsing errors coherent per language idiom |
| `protocol.check_*` validators | `protocol::check_*` validators | single definition used by `Mouse` and `BatchBuilder` |
| `BatchBuilder` full setter set | `BatchBuilder` full setter set | identical coverage, identical validation |
| `errors.ConnectionError` shadowing builtin | `MakcuConnectionError` etc. | prefixed to avoid shadowing `std` |
| `config.PROMPT = b">>> "` | `makcu_serial::PROMPT` | single prompt constant, no duplication |
