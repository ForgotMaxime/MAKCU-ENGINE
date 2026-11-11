"""UDP-to-serial relay (example only, not for production use).

Listens on UDP :5005 and relays a tiny text protocol to the MAKCU:

- ``MOVE:dx,dy`` -> relative move
- ``WHEEL:n`` -> wheel delta
- ``PRESS:LEFT|RIGHT`` / ``RELEASE:LEFT|RIGHT`` -> button down/up
- ``CLICK:LEFT|RIGHT`` -> scheduled click
- ``PING`` -> replies ``PONG`` without touching serial

Only move/wheel/press/release commands are ever forwarded; settings,
masks, locks, screen, autorelease, and stream commands are never sent,
so persistent device settings cannot change from the network.
Requires an explicit ``--port``; never auto-detects.
"""

from __future__ import annotations

import argparse
import socket

from makcu_engine import MakcuTransport, Mouse
from makcu_engine.types import Button

LISTEN_PORT = 5005

_BUTTONS = {"LEFT": Button.LEFT, "RIGHT": Button.RIGHT}


def parse_datagram(text: str) -> tuple[str, object] | None:
    """Parse one UDP payload into an action tuple; None when invalid."""
    msg = text.strip()
    if msg == "PING":
        return ("ping", ())
    if msg.startswith("MOVE:"):
        try:
            xs, ys = msg[5:].split(",", 1)
            return ("move", (int(xs.strip()), int(ys.strip())))
        except ValueError:
            return None
    if msg.startswith("WHEEL:"):
        try:
            return ("wheel", (int(msg[6:].strip()),))
        except ValueError:
            return None
    if msg.startswith("PRESS:"):
        btn = _BUTTONS.get(msg[6:].strip().upper())
        return ("down", (btn,)) if btn is not None else None
    if msg.startswith("RELEASE:"):
        btn = _BUTTONS.get(msg[8:].strip().upper())
        return ("up", (btn,)) if btn is not None else None
    if msg.startswith("CLICK:"):
        btn = _BUTTONS.get(msg[6:].strip().upper())
        return ("click", (btn,)) if btn is not None else None
    return None


def apply_action(mouse: Mouse, action: tuple[str, object]) -> str | None:
    """Forward one parsed action to the mouse; returns a UDP reply or None."""
    kind, payload = action
    if kind == "ping":
        return "PONG"
    if kind == "move":
        dx, dy = payload  # type: ignore[misc]
        mouse.move(int(dx), int(dy))
        return None
    if kind == "wheel":
        (delta,) = payload  # type: ignore[misc]
        mouse.wheel(int(delta))
        return None
    if kind == "down":
        (btn,) = payload  # type: ignore[misc]
        assert isinstance(btn, Button)
        mouse.down(btn)
        return None
    if kind == "up":
        (btn,) = payload  # type: ignore[misc]
        assert isinstance(btn, Button)
        mouse.up(btn)
        return None
    if kind == "click":
        (btn,) = payload  # type: ignore[misc]
        assert isinstance(btn, Button)
        mouse.click(btn)
        return None
    return None


def build_parser() -> argparse.ArgumentParser:
    """Build the relay CLI parser (explicit --port, no auto-detect)."""
    p = argparse.ArgumentParser(description="UDP :5005 -> serial relay (example only)")
    p.add_argument("--port", required=True, help="Serial port, e.g. COM4 or /dev/ttyUSB0")
    p.add_argument("--baud", type=int, default=4000000, help="Baud rate (default 4000000)")
    p.add_argument("--listen", type=int, default=LISTEN_PORT, help="UDP port (default 5005)")
    return p


def main(argv: list[str] | None = None) -> int:
    """Run the UDP relay until interrupted."""
    args = build_parser().parse_args(argv)
    with MakcuTransport(args.port, args.baud) as t:
        mouse = Mouse(t)
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        try:
            sock.bind(("127.0.0.1", args.listen))
            print(f"relay 127.0.0.1:{args.listen} -> {args.port} (Ctrl+C to stop)")
            while True:
                data, addr = sock.recvfrom(1024)
                try:
                    text = data.decode("ascii")
                except UnicodeDecodeError:
                    continue
                action = parse_datagram(text)
                if action is None:
                    continue
                reply = apply_action(mouse, action)
                if reply is not None:
                    sock.sendto(reply.encode("ascii"), addr)
        except KeyboardInterrupt:
            return 0
        finally:
            sock.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
