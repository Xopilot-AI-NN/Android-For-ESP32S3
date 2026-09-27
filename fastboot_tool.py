#!/usr/bin/env python3
from __future__ import annotations

import argparse
import glob
import sys
import time

try:
    import serial
    import serial.tools.list_ports
except ImportError:
    print("Install pyserial: pip install pyserial", file=sys.stderr)
    raise SystemExit(2)


def auto_port() -> str | None:
    for p in serial.tools.list_ports.comports():
        text = f"{p.description or ''} {p.manufacturer or ''}".lower()
        if any(x in text for x in ("esp32", "cdc", "serial", "usb")):
            return p.device
    for pat in ("/dev/ttyACM*", "/dev/ttyUSB*", "/dev/cu.usbmodem*"):
        xs = glob.glob(pat)
        if xs:
            return xs[0]
    return None


def transact(port: str, command: str, baud: int, timeout: float) -> int:
    # Configure modem-control state *before* opening the port. Creating
    # Serial(port, ...) opens immediately and can briefly assert DTR/RTS on
    # some hosts, which is exactly what the no-reset desktop viewer avoids.
    s = serial.Serial()
    s.port = port
    s.baudrate = baud
    s.timeout = 0.1
    s.dtr = False
    s.rts = False
    try:
        s.open()
        time.sleep(0.15)
        s.reset_input_buffer()
        s.write((command + "\n").encode())
        s.flush()
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            raw = s.readline()
            if not raw:
                continue
            line = raw.decode(errors="replace").strip()
            if line.startswith("INFO"):
                print(line[4:])
            elif line.startswith("OKAY"):
                if line[4:]:
                    print(line[4:])
                return 0
            elif line.startswith("FAIL"):
                print("FAIL:", line[4:], file=sys.stderr)
                return 1
        print("timeout", file=sys.stderr)
        return 1
    finally:
        if s.is_open:
            s.close()


def main() -> int:
    ap = argparse.ArgumentParser(description="AOSP Wear OS fastboot+ serial client")
    ap.add_argument("command", nargs="+", help="wire command, e.g. getvar:all")
    ap.add_argument("--port", "-p")
    ap.add_argument("--baud", "-b", type=int, default=115200)
    ap.add_argument("--timeout", "-t", type=float, default=3.0)
    args = ap.parse_args()
    port = args.port or auto_port()
    if not port:
        print("No ESP32 serial port found", file=sys.stderr)
        return 2
    return transact(port, " ".join(args.command), args.baud, args.timeout)


if __name__ == "__main__":
    raise SystemExit(main())
