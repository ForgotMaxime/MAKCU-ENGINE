"""In-memory transport. Same interface as serial, for tests and offline dev."""

from __future__ import annotations

from .errors import MakcuProtocolError, MakcuTimeoutError


class MockTransport:
    """In-memory Transport for tests and offline development."""
    def __init__(self, replies: dict[str, str] | None = None) -> None:
        self.replies = replies or {}
        self.sent: list[str] = []
        self._stream = bytearray()
        self.fail_next_send = False
        self._loop_active = False

    def push_stream_frame(self, frame: bytes) -> None:
        """Enqueue a raw stream frame for read_line / poll_event."""
        self._stream += frame

    def _lookup(self, name: str, args: tuple[int | str, ...]) -> str:
        cmd = f"{name}({','.join(str(a) for a in args)})"
        self.sent.append(cmd)
        base = cmd.split("(")[0]
        for key in (cmd, base, name):
            if key in self.replies:
                return self.replies[key]
        if base.startswith("km.lock_"):
            return "0"
        if base.split(".")[-1] in ("left", "right", "middle", "side1", "side2"):
            return "0"
        return {
            "km.version": "km.MAKCU",
            "km.device": "R:MK;M:16uf;K:8uf;C:0uf",
            "km.echo": "0",
            "km.moving": "0",
            "km.buttons": "0",
            "km.left": "0",
            "km.autorelease": "0",
            "km.api_version": "1",
            "km.getpos": "km.getpos(100,200)",
            "km.screen": "km.screen(1920,1080)",
        }.get(base, "")

    def query(self, name: str, *args: int | str) -> str:
        """Canned reply lookup; records the command."""
        return self._lookup(name, args)

    def set(self, name: str, *args: int | str) -> None:
        """Record a setter; honors the fail-next flag."""
        if self.fail_next_send:
            self.fail_next_send = False
            raise MakcuProtocolError(f"{name} failed: device ERR: {name!r}")
        self._lookup(name, args)

    def fire_and_forget(self, name: str, *args: int | str) -> None:
        """Record without reply."""
        self.sent.append(f"{name}({','.join(str(a) for a in args)})")

    def send_raw(self, data: bytes) -> None:
        """Record raw bytes; honors the fail-next flag."""
        if self.fail_next_send:
            self.fail_next_send = False
            raise MakcuProtocolError(f"device ERR: {data[:32]!r}")
        self.sent.append(data.decode("ascii"))

    def read_line(self, timeout_s: float = 0.5) -> bytes:
        """Pop one CRLF-terminated frame from the queued stream bytes."""
        idx = self._stream.find(b"\r\n")
        if idx < 0:
            raise MakcuTimeoutError("no stream frame")
        line = bytes(self._stream[: idx + 2])
        del self._stream[: idx + 2]
        return line

    def read_exact(self, n: int, timeout_s: float = 0.5) -> bytes:
        """Pop exactly n bytes from the queued stream bytes."""
        if n <= 0:
            raise MakcuProtocolError(f"read_exact n={n}")
        if len(self._stream) < n:
            raise MakcuTimeoutError("short stream read")
        out = bytes(self._stream[:n])
        del self._stream[:n]
        return out

    def start_event_loop(self) -> None:
        """Mark the loop running (no thread; mock pops frames directly)."""
        self._loop_active = True

    def stop_event_loop(self, timeout_s: float = 1.0) -> None:
        """Mark the loop stopped."""
        self._loop_active = False

    def event_loop_running(self) -> bool:
        """Whether the mock loop flag is set."""
        return self._loop_active

    def poll_event(self, timeout_s: float = 0.5) -> bytes:
        """Pop the next queued stream frame."""
        return self.read_line(timeout_s)
