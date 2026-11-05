"""Button event monitor: background pump thread + change callbacks + drain queue.

Works on both paths: with the transport event loop running (polls never
block queries) or on the legacy direct-poll path (no queries in flight).
"""

from __future__ import annotations

import threading
from collections.abc import Callable

from .errors import MakcuTimeoutError
from .mouse import Mouse


class ButtonMonitor:
    """Background button-event pump: change callbacks plus drain queue."""
    def __init__(self, mouse: Mouse, poll_s: float = 0.05) -> None:
        self._mouse = mouse
        self._poll_s = poll_s
        self._stop = threading.Event()
        self._cbs: list[Callable[[int], None]] = []
        self._pending: list[int] = []
        self._last: int | None = None
        self._thread = threading.Thread(target=self._run, daemon=True, name="makcu-monitor")

    def on_change(self, cb: Callable[[int], None]) -> Callable[[int], None]:
        """Register a callback invoked on each mask change."""
        self._cbs.append(cb)
        return cb

    def start(self, mode: int = 1) -> None:
        """Enable the button stream and start the pump thread."""
        self._mouse.start_button_stream(mode)
        self._stop.clear()
        if not self._thread.is_alive():
            self._thread.start()

    def stop(self) -> None:
        """Stop the pump thread and disable the button stream."""
        self._stop.set()
        if self._thread.is_alive():
            self._thread.join(1.0)
        try:
            self._mouse.stop_button_stream()
        except MakcuTimeoutError:
            pass

    def pump_once(self, timeout_s: float = 0.2) -> int | None:
        """Poll once, dispatch on change. Testable without threads."""
        try:
            mask = self._mouse.poll_button_event(timeout_s)
        except MakcuTimeoutError:
            return None
        if mask != self._last:
            self._last = mask
            self._pending.append(mask)
            for cb in list(self._cbs):
                cb(mask)
        return mask

    def drain(self) -> list[int]:
        """Take all pending change events since the last drain."""
        out, self._pending = self._pending, []
        return out

    def _run(self) -> None:
        while not self._stop.is_set():
            self.pump_once(self._poll_s)
