#!/usr/bin/env python3
"""AOSP Wear OS Debug Bridge for the no-reset development transport.

The local Unix bridge is still useful while bringing up real wireless ADB.  In
v0.6 it also exposes connectivity commands so Wi-Fi/BLE can be tested without
adding temporary buttons to SystemUI.
"""
from __future__ import annotations

import argparse
import os
import socket
import time
from pathlib import Path


def default_socket() -> Path:
    runtime = os.environ.get("XDG_RUNTIME_DIR")
    if runtime:
        return Path(runtime) / "aosp-wear-viewer.sock"
    return Path(f"/tmp/aosp-wear-viewer-{os.getuid()}.sock")


SIMPLE_COMMANDS = {
    "getprop": ("@ZADB|GETPROP", "getprop"),
    "services": ("@ZADB|SERVICES", "services"),
    "packages": ("@ZADB|PACKAGES", "packages"),
    "pm": ("@ZADB|PACKAGES", "packages"),
    "dumpsys": ("@ZADB|DUMPSYS", "dumpsys"),
    "wifi-scan": ("@ZADB|WIFI_SCAN", "wifi"),
    "wifi-status": ("@ZADB|WIFI_STATUS", "wifi"),
    "wifi-disconnect": ("@ZADB|WIFI_DISCONNECT", "wifi"),
    "bt-scan": ("@ZADB|BT_SCAN", "bt"),
    "bt-status": ("@ZADB|BT_STATUS", "bt"),
    "bt-disconnect": ("@ZADB|BT_DISCONNECT", "bt"),
}


def build_command(args: argparse.Namespace) -> tuple[str, str]:
    if args.command in SIMPLE_COMMANDS:
        return SIMPLE_COMMANDS[args.command]
    if args.command == "wifi-connect":
        ssid = args.ssid.encode("utf-8")
        password = args.password.encode("utf-8")
        if not (1 <= len(ssid) <= 32):
            raise SystemExit("SSID must be 1..32 UTF-8 bytes")
        if len(password) > 64:
            raise SystemExit("password must be <=64 UTF-8 bytes")
        return f"@ZADB|WIFI_CONNECT|{ssid.hex()}|{password.hex()}", "wifi"
    if args.command == "bt-connect":
        compact = args.address.replace(":", "").replace("-", "").lower()
        if len(compact) != 12 or any(ch not in "0123456789abcdef" for ch in compact):
            raise SystemExit("Bluetooth address must look like AA:BB:CC:DD:EE:FF")
        return f"@ZADB|BT_CONNECT|{compact}", "bt"
    if args.command == "companion-time":
        try:
            hh, mm = args.time.split(":", 1)
            minutes = int(hh) * 60 + int(mm)
        except (ValueError, AttributeError):
            raise SystemExit("time must be HH:MM")
        if not (0 <= minutes < 1440 and 0 <= int(mm) < 60):
            raise SystemExit("time must be HH:MM in 24-hour format")
        return f"@ZADB|COMPANION_TIME|{minutes}", "time"
    raise AssertionError(args.command)


def main() -> int:
    ap = argparse.ArgumentParser(description="AOSP Wear OS Android 17 QPR1 debug bridge")
    sub = ap.add_subparsers(dest="command", required=True)
    for name in SIMPLE_COMMANDS:
        sub.add_parser(name)
    wifi_connect = sub.add_parser("wifi-connect")
    wifi_connect.add_argument("ssid")
    wifi_connect.add_argument("password", nargs="?", default="")
    bt_connect = sub.add_parser("bt-connect")
    bt_connect.add_argument("address")
    companion_time = sub.add_parser("companion-time", help="explicitly simulate paired-phone time; never sent automatically")
    companion_time.add_argument("time", help="24-hour HH:MM")
    ap.add_argument("--socket", type=Path, default=default_socket())
    ap.add_argument("--timeout", type=float, default=8.0)
    args = ap.parse_args()

    command, wanted = build_command(args)
    if not args.socket.exists():
        raise SystemExit(f"bridge socket not found: {args.socket}\nStart ./flash first.")

    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.settimeout(0.2)
    sock.connect(str(args.socket))
    sock.sendall((command + "\n").encode("ascii"))

    deadline = time.monotonic() + args.timeout
    buf = bytearray()
    try:
        while time.monotonic() < deadline:
            try:
                chunk = sock.recv(4096)
            except socket.timeout:
                continue
            if not chunk:
                break
            buf.extend(chunk)
            while b"\n" in buf:
                raw, _, rest = buf.partition(b"\n")
                buf[:] = rest
                line = raw.rstrip(b"\r").decode("utf-8", errors="replace")
                prefix = f"@ZADB|OUT|{wanted}|"
                if line.startswith(prefix):
                    print(line[len(prefix):].replace("; ", "\n"))
                    return 0
    finally:
        sock.close()

    raise SystemExit(f"no ZADB response within {args.timeout:.1f}s")


if __name__ == "__main__":
    raise SystemExit(main())
