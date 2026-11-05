"""In-memory transport. Same interface as serial, for tests and offline dev."""

from __future__ import annotations


class MockTransport:
    def __init__(self, replies: dict[str, str] | None = None) -> None:
        self.replies = replies or {}
        self.sent: list[str] = []

    def _lookup(self, name: str, args: tuple[int | str, ...]) -> str:
        cmd = f"{name}({','.join(str(a) for a in args)})"
        self.sent.append(cmd)
        base = cmd.split("(")[0]
        for key in (cmd, base, name):
            if key in self.replies:
                return self.replies[key]
        return {
            "km.version": "km.MAKCU",
            "km.device": "R:MK;M:16uf;K:8uf;C:0uf",
            "km.echo": "0",
            "km.moving": "0",
            "km.left": "0",
            "km.getpos": "km.getpos(100,200)",
            "km.screen": "km.screen(1920,1080)",
        }.get(base, "")

    def query(self, name: str, *args: int | str) -> str:
        return self._lookup(name, args)

    def set(self, name: str, *args: int | str) -> None:
        self._lookup(name, args)

    def fire_and_forget(self, name: str, *args: int | str) -> None:
        self.sent.append(f"{name}({','.join(str(a) for a in args)})")
