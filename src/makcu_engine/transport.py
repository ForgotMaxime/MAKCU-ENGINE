"""Exclusive serial transport. Single reader/writer, thread-safe.

Port and baud are never hardcoded in the flow: use :meth:`detect` to pick
``(port, baud)`` at runtime, then open explicitly. :meth:`auto` is the
one-call shortcut (detect + connect).
"""

from __future__ import annotations

import threading

import serial
import serial.tools.list_ports

from . import config
from .errors import MakcuConnectionError, MakcuProtocolError, MakcuTimeoutError
from .protocol import build_cmd, strip_prompt


def list_candidates() -> list[str]:
    """Order ports: CH343 VID/PID match first, other USB serial, then rest."""
    try:
        infos = list(serial.tools.list_ports.comports())
    except Exception:
        return []
    scored: list[tuple[int, str]] = []
    for info in infos:
        name = info.device
        if info.vid == config.USB_VID and info.pid == config.USB_PID:
            score = 0
        elif info.vid is not None:
            score = 1
        elif "USB" in (info.description or "").upper() or "SERIAL" in (info.description or "").upper():
            score = 2
        else:
            score = 3
        scored.append((score, name))
    scored.sort()
    return [name for _, name in scored]


class MakcuTransport:
    def __init__(self, port: str, baudrate: int, timeout_s: float = config.TIMEOUT_S) -> None:
        self._port = port
        self._baudrate = baudrate
        self._timeout_s = timeout_s
        self._lock = threading.Lock()
        self._ser: serial.Serial | None = None

    @property
    def port(self) -> str:
        return self._port

    @property
    def baudrate(self) -> int:
        return self._baudrate

    @property
    def is_connected(self) -> bool:
        return self._ser is not None

    def connect(self, retries: int = 10) -> None:
        last: Exception | None = None
        for attempt in range(retries):
            try:
                self._ser = serial.Serial(
                    self._port,
                    baudrate=self._baudrate,
                    bytesize=8,
                    parity="N",
                    stopbits=1,
                    timeout=self._timeout_s,
                    write_timeout=self._timeout_s,
                    xonxoff=False,
                    rtscts=False,
                    dsrdtr=False,
                    exclusive=True,
                )
                self._ser.reset_input_buffer()
                self._ser.reset_output_buffer()
                return
            except serial.SerialException as e:
                last = e
                import time as _time

                _time.sleep(0.2 * (attempt + 1))
        raise MakcuConnectionError(f"cannot open {self._port}@{self._baudrate}: {last}") from last

    def close(self) -> None:
        if self._ser is not None:
            try:
                self._ser.close()
            finally:
                self._ser = None

    def __enter__(self) -> MakcuTransport:
        if self._ser is None:
            self.connect()
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()

    @classmethod
    def probe(cls, port: str, timeout_s: float = config.TIMEOUT_S) -> int:
        import time as _time

        for baud in config.BAUDS:
            ser = None
            try:
                ser = serial.Serial(port, baudrate=baud, timeout=timeout_s,
                                    write_timeout=timeout_s, exclusive=True)
                ser.reset_input_buffer()
                ser.write(build_cmd("km.version"))
                ser.flush()
                raw = ser.read_until(config.PROMPT)
                if b"km.MAKCU" in raw:
                    return baud
            except serial.SerialException:
                continue
            finally:
                if ser is not None:
                    try:
                        ser.close()
                    finally:
                        _time.sleep(0.15)
        raise MakcuConnectionError(f"no MAKCU on {port}")

    @classmethod
    def detect(cls, timeout_s: float = config.TIMEOUT_S) -> tuple[str, int]:
        for port in list_candidates():
            try:
                return port, cls.probe(port, timeout_s)
            except MakcuConnectionError:
                continue
        raise MakcuConnectionError("no MAKCU detected on any serial port")

    @classmethod
    def auto(cls, timeout_s: float = config.TIMEOUT_S) -> MakcuTransport:
        import time as _time

        port, baud = cls.detect(timeout_s)
        _time.sleep(0.3)
        t = cls(port, baud, timeout_s)
        t.connect()
        return t

    def _exchange(self, payload: bytes) -> bytes:
        if self._ser is None:
            raise MakcuConnectionError("transport not connected")
        with self._lock:
            self._ser.reset_input_buffer()
            self._ser.write(payload)
            self._ser.flush()
            raw = self._ser.read_until(config.PROMPT)
        if not raw:
            raise MakcuTimeoutError("no reply (prompt >>> missing)")
        if config.PROMPT not in raw:
            raise MakcuProtocolError(f"truncated reply: {raw!r}")
        return raw

    def query(self, name: str, *args: int | str) -> str:
        raw = self._exchange(build_cmd(name, *args))
        try:
            return strip_prompt(raw)
        except ValueError as e:
            raise MakcuProtocolError(str(e)) from e

    def set(self, name: str, *args: int | str, error_window_s: float = 0.08) -> None:
        if self._ser is None:
            raise MakcuConnectionError("transport not connected")
        with self._lock:
            self._ser.reset_input_buffer()
            self._ser.write(build_cmd(name, *args))
            self._ser.flush()
            prev = self._ser.timeout
            try:
                self._ser.timeout = error_window_s
                raw = self._ser.read_until(config.PROMPT)
            finally:
                self._ser.timeout = prev
        if b"ERR" in raw:
            raise MakcuProtocolError(f"{name} rejected: {raw!r}")

    def fire_and_forget(self, name: str, *args: int | str) -> None:
        if self._ser is None:
            raise MakcuConnectionError("transport not connected")
        with self._lock:
            self._ser.reset_output_buffer()
            self._ser.write(build_cmd(name, *args))
            self._ser.flush()
