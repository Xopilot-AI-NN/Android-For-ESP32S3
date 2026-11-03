#!/usr/bin/env python3
# Модуль чтения и фильтрации журналов.
"""Watch logs over the existing PC bridge or exclusive, no-reset serial.

Times without a device uptime are host receive times. This is the project's
ZADB log frontend, not the Android adbd wire protocol.
"""
from __future__ import annotations

import argparse
from collections import deque
from dataclasses import dataclass
from datetime import datetime
import json
import re
import socket
import time
from pathlib import Path

# Параметр LEVELS компонента чтения и фильтрации журналов.
LEVELS = "VDIWEFS"
# Параметр ANSI компонента чтения и фильтрации журналов.
ANSI = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
# Параметр PROTOCOL компонента чтения и фильтрации журналов.
PROTOCOL = re.compile(r"^@?(?:ZWUI|ZADB|ZBLK|HOST)\|")
# Параметр IDF компонента чтения и фильтрации журналов.
IDF = re.compile(r"^([EWIDV]) \((\d+)\) ([^:]+):\s*(.*)$")


@dataclass(frozen=True)
# Разобранная запись журнала или пользовательского списка компонента чтения и фильтрации журналов.
class Record:
    received: float
    level: str
    tag: str
    message: str
    uptime_ms: int | None = None

    # Кодирует запись журнала в строку протокола @ZLOG.
    def wire(self) -> str:
        return "@ZLOG|" + json.dumps(self.__dict__, ensure_ascii=False, separators=(",", ":"))

    @classmethod
    # Проверяет и восстанавливает запись журнала из строки @ZLOG.
    def from_wire(cls, line: str) -> Record | None:
        if not line.startswith("@ZLOG|"):
            return None
        try:
            return cls(**json.loads(line[6:]))
        except (TypeError, ValueError):
            return None


# Разбирает реальную строку лога, отбрасывая управляющие пакеты и ANSI-коды.
def parse_record(line: str, received: float | None = None) -> Record | None:
    text = ANSI.sub("", line).strip()
    if not text or PROTOCOL.match(text) or text.startswith("@ZLOG|"):
        return None
    timestamp = time.time() if received is None else received
    match = IDF.match(text)
    if match:
        level, uptime, tag, message = match.groups()
        return Record(timestamp, level, tag[:32], message[:512], int(uptime))
    tag, sep, message = text.partition(":")
    if not sep or len(tag) > 32:
        tag, message = "Watch", text
    low = text.lower()
    level = "E" if any(x in low for x in ("error", "failed", "panic", "refused")) else (
        "W" if any(x in low for x in ("warning", "timeout")) else "I")
    return Record(timestamp, level, tag.strip(), message.strip()[:512])


class LogBuffer:
    """Host-side bounded history; callers synchronize access with bridge lock."""
    # Создаёт объект и подготавливает принадлежащие ему буферы/подключения.
    def __init__(self) -> None:
        self.records: deque[Record] = deque(maxlen=128)

    # Добавляет разобранную запись в ограниченную историю.
    def add(self, line: str) -> Record | None:
        record = parse_record(line)
        if record is not None:
            self.records.append(record)
        return record


# Проверяет фильтры тегов и уровней журнала.
def filters(specs: list[str], silent: bool) -> dict[str, str]:
    result = {"*": "S" if silent else "I"}
    for spec in specs:
        tag, sep, level = spec.partition(":")
        level = level.upper() if sep else "V"
        if not tag or level not in LEVELS or len(level) != 1:
            raise ValueError(f"invalid filter {spec!r}; use Tag:D or '*:W'")
        result[tag] = level
    return result


# Сверяет запись с выбранным порогом уровня и тегом.
def accepted(record: Record, rules: dict[str, str]) -> bool:
    threshold = rules.get(record.tag, rules["*"])
    return threshold != "S" and LEVELS.index(record.level) >= LEVELS.index(threshold)


# Форматирует журнал без вымышленных PID/TID устройства.
def format_record(record: Record, style: str) -> str:
    if style == "raw":
        return record.message
    stamp = datetime.fromtimestamp(record.received).strftime("%m-%d %H:%M:%S.%f")[:-3]
    # ESP32 firmware has no Android process/thread IDs; never invent them.
    if style == "threadtime":
        return f"{stamp}     0     0 {record.level} {record.tag:<16}: {record.message}"
    if style == "time":
        return f"{stamp} {record.level}/{record.tag}(0): {record.message}"
    return f"{record.level}/{record.tag}(0): {record.message}"


# Разбирает параметры запуска и выполняет основной сценарий утилиты.
def main(argv: list[str] | None = None) -> int:
    from usb_block_server import auto_port, default_viewer_socket, open_serial
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("-d", action="store_true", help="dump PC bridge history and exit")
    ap.add_argument("-c", action="store_true", help="clear PC bridge history")
    ap.add_argument("-t", type=int, metavar="LINES", help="dump last LINES (max 128)")
    ap.add_argument("-v", choices=("threadtime", "time", "brief", "raw"), default="threadtime")
    ap.add_argument("-s", action="store_true", help="silence unspecified tags")
    ap.add_argument("--serial", metavar="PORT", help="direct SD-mode serial, no reset")
    ap.add_argument("--socket", type=Path, default=default_viewer_socket())
    ap.add_argument("specs", nargs="*", help="Tag:priority filters, e.g. 'radio:D' '*:W'")
    ap.add_argument("--elf", type=Path, help="decode ESP backtraces with the exact Firmware ELF")
    args = ap.parse_args(argv)
    decoder = None
    if args.elf:
        from tools.decode_backtrace import Decoder
        decoder = Decoder(args.elf)
    try:
        rules = filters(args.specs, args.s)
    except ValueError as error:
        ap.error(str(error))
    if args.t is not None and not 1 <= args.t <= 128:
        ap.error("-t must be 1..128")

    # Добавляет запись в журнал или выводит отфильтрованную строку.
    def emit(record: Record | None) -> None:
        if record is not None and accepted(record, rules):
            print(format_record(record, args.v), flush=True)
            if decoder is not None:
                for symbol in decoder.decode(record.message):
                    print(f"  {symbol}", flush=True)

    try:
        if not args.serial and args.socket.exists():
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
                sock.connect(str(args.socket))
                command = "CLEAR" if args.c else "DUMP"
                sock.sendall(f"@ZLOG|{command}|{args.t or 128}\n".encode())
                with sock.makefile("r", encoding="utf-8", errors="replace") as lines:
                    for line in lines:
                        if line.strip() == "@ZLOG|END":
                            if args.c or args.d or args.t is not None:
                                return 0
                        else:
                            emit(Record.from_wire(line.strip()))
            return 0
        if args.d or args.c or args.t is not None:
            ap.error("history is stored by the PC bridge; direct serial supports live logs only")
        with open_serial(args.serial or auto_port(), 0.2) as ser:
            print("# Live USB logs; timestamps are host receive time. Ctrl+C stops without reset.", flush=True)
            while True:
                raw = ser.readline(2048)
                if raw:
                    emit(parse_record(raw.decode("utf-8", errors="replace")))
    except KeyboardInterrupt:
        return 0
    except (OSError, socket.error) as error:
        raise SystemExit(f"logcat transport: {error}") from error


if __name__ == "__main__":
    raise SystemExit(main())
