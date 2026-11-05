"""Transport interface. Mouse depends on this, never on serial directly."""

from __future__ import annotations

from typing import Protocol


class Transport(Protocol):
    def query(self, name: str, *args: int | str) -> str: ...
    def set(self, name: str, *args: int | str) -> None: ...
    def fire_and_forget(self, name: str, *args: int | str) -> None: ...
