"""Button event monitor: background pump thread + change callbacks + drain queue.

Works on both paths: with the transport event loop running (polls never
block queries) or on the legacy direct-poll path (no queries in flight).

Lifecycle is explicit: start() on a running monitor raises, stop() is
always safe, and stop() then start() cleanly restarts with a fresh thread.
"""

from __future__ import annotations

import logging
import threading
from collections.abc import Callable

from .errors import MakcuProtocolError, MakcuTimeoutError
from .mouse import Mouse

logger = logging.getLogger(__name__)


class ButtonMonitor:
    """Background button-event pump: change callbacks plus drain queue."""

    def __init__(self, mouse: Mouse, poll_s: float = 0.05) -> None:
        self._mouse = mouse
        self._poll_s = poll_s
        self._stop = threading.Event()
        self._cbs: list[Callable[[int], None]] = []
        self._cbs_lock = threading.Lock()
        self._pending: list[int] = []
        self._last: int | None = None
        self._thread: threading.Thread | None = None
        self._state_lock = threading.Lock()
        self._pump_errors = 0

    def on_change(self, cb: Callable[[int], None]) -> Callable[[int], None]:
        """Register a callback invoked on each mask change."""
        with self._cbs_lock:
            self._cbs.append(cb)
        return cb

    @property
    def is_running(self) -> bool:
        """Whether the pump thread is currently alive."""
        with self._state_lock:
            return self._thread is not None and self._thread.is_alive()

    @property
    def pump_errors(self) -> int:
        """Count of pump iterations that raised and were counted, not fatal."""
        with self._state_lock:
            return self._pump_errors

    def start(self, mode: int = 1) -> None:
        """Enable the button stream and start the pump thread.

        Raises if the monitor is already running; stop() first to restart.
        """
        with self._state_lock:
            if self._thread is not None and self._thread.is_alive():
                raise MakcuProtocolError("monitor already started")
            self._mouse.start_button_stream(mode)
            self._stop.clear()
            self._thread = threading.Thread(target=self._run, daemon=True, name="makcu-monitor")
            self._thread.start()

    def stop(self) -> None:
        """Stop the pump thread and disable the button stream. Always safe."""
        with self._state_lock:
            thread, self._thread = self._thread, None
        self._stop.set()
        if thread is not None and thread is not threading.current_thread():
            if thread.is_alive():
                thread.join(1.0)
            if thread.is_alive():
                logger.warning("monitor thread still alive after join")
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
        with self._cbs_lock:
            cbs = list(self._cbs)
        with self._state_lock:
            if mask == self._last:
                return mask
            self._last = mask
            self._pending.append(mask)
        for cb in cbs:
            cb(mask)
        return mask

    def drain(self) -> list[int]:
        """Take all pending change events since the last drain."""
        with self._state_lock:
            out, self._pending = self._pending, []
            return out

    def _run(self) -> None:
        """Pump until stopped; per-iteration errors are counted, never fatal."""
        while not self._stop.is_set():
            try:
                self.pump_once(self._poll_s)
            except Exception:
                with self._state_lock:
                    self._pump_errors += 1
                logger.exception("monitor pump iteration failed")
                continue
