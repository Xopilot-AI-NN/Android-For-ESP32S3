#!/usr/bin/env python3
# Модуль клиента fastboot-протокола проекта.
from __future__ import annotations

import argparse
import glob
import sys
import time
import os
import shutil
import subprocess
from pathlib import Path

try:
    import serial
    import serial.tools.list_ports
except ImportError:
    print("Install pyserial: pip install pyserial", file=sys.stderr)
    raise SystemExit(2)


# Выбирает доступный последовательный порт ESP32-S3.
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


# Выполняет одну команду fastboot с ограниченным ожиданием ответа.
def transact(port: str, command: str, baud: int, timeout: float) -> int:
    # Configure modem-control state *before* opening the port. Creating
    # Serial(port, ...) opens immediately and can briefly assert DTR/RTS on
    # some hosts, which is exactly what the no-reset desktop viewer avoids.
    s = serial.Serial()
    s.port = port
    s.baudrate = baud
    s.timeout = 0.1
    if os.name == "posix":
        # Same advisory TTY ownership as usb_block_server.py: fail instead of
        # allowing a second reader to consume the block protocol's replies.
        s.exclusive = True
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


# Разбирает параметры запуска и выполняет основной сценарий утилиты.
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
    if args.command[0] == "update":
        # Native code runs from internal SPI flash, so use Espressif's actual
        # flashing protocol instead of pretending a GPT boot.img is executable.
        if len(args.command) != 2:
            ap.error("update requires a Firmware ELF path")
        image = Path(args.command[1]).resolve()
        with image.open("rb") as source:
            if source.read(4) != b"\x7fELF":
                ap.error("update expects the Firmware ELF, not a GPT/SD image")
        espflash = shutil.which("espflash")
        if espflash is None:
            ap.error("espflash is not installed")
        # The probe closes serial before espflash opens it. A running PC block
        # server must be stopped first; never steal its open device descriptor.
        result = transact(port, "getvar:version", args.baud, args.timeout)
        if result:
            return result
        return subprocess.run([
            espflash, "flash", "--chip", "esp32s3", "--flash-size", "4mb",
            "--partition-table", str(Path(__file__).with_name("partitions.csv")),
            "--port", port, str(image),
        ], check=False).returncode
    return transact(port, " ".join(args.command), args.baud, args.timeout)


if __name__ == "__main__":
    raise SystemExit(main())
