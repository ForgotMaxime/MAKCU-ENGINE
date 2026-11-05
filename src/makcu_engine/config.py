"""Shared discovery parameters. No selected port/baud here — detect at runtime."""

BAUDS = (4_000_000, 115_200)
USB_VID = 0x1A86
USB_PID = 0x55D3
TIMEOUT_S = 0.8
PROMPT = b">>> "
