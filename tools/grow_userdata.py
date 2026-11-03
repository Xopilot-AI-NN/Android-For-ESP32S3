#!/usr/bin/env python3
# Модуль расширения userdata без перемещения остальных разделов.
"""Grow the final raw userdata partition, keeping all existing partition starts/data.

Defaults to a read-only plan. --apply saves GPT sectors before writing metadata.
No filesystem is formatted or resized: current userdata uses raw reserved ranges.
"""
from __future__ import annotations

import argparse
import datetime
import fcntl
import json
import os
from pathlib import Path
import stat
import struct
import sys
import zlib

# Размер логического сектора, байт.
SECTOR = 512
# Параметр EXPECTED компонента расширения userdata без перемещения остальных разделов.
EXPECTED = ["misc", "metadata", "boot_a", "boot_b", "init_boot_a", "init_boot_b",
            "vendor_boot_a", "vendor_boot_b", "vbmeta_a", "vbmeta_b", "super", "swap", "userdata"]


# Читает заданный диапазон файла/устройства и проверяет полноту чтения.
def read_at(disk, offset: int, size: int) -> bytes:
    disk.seek(offset)
    data = disk.read(size)
    if len(data) != size:
        raise ValueError("short disk read")
    return data


# Проверяет структуру и CRC заголовка GPT.
def check_header(header: bytes) -> None:
    if header[:8] != b"EFI PART" or len(header) != SECTOR:
        raise ValueError("no GPT signature")
    size = struct.unpack_from("<I", header, 12)[0]
    if not 92 <= size <= SECTOR:
        raise ValueError("invalid GPT header size")
    copy = bytearray(header[:size])
    checksum = struct.unpack_from("<I", copy, 16)[0]
    copy[16:20] = bytes(4)
    if zlib.crc32(copy) != checksum:
        raise ValueError("GPT header checksum mismatch; repair the GPT first")


# Собирает новый GPT-заголовок с актуальными границами и CRC.
def make_header(original: bytes, current: int, backup: int, last: int,
                entries_lba: int, entries_crc: int) -> bytes:
    result = bytearray(original)
    struct.pack_into("<QQ", result, 24, current, backup)
    struct.pack_into("<Q", result, 48, last)
    struct.pack_into("<Q", result, 72, entries_lba)
    struct.pack_into("<I", result, 88, entries_crc)
    result[16:20] = bytes(4)
    size = struct.unpack_from("<I", result, 12)[0]
    struct.pack_into("<I", result, 16, zlib.crc32(result[:size]))
    return bytes(result)


# Отказывает в работе с устройством, которое уже используется системой.
def refuse_in_use(path: Path, mode: int) -> None:
    if not stat.S_ISBLK(mode):
        return
    device = path.stat().st_rdev
    tree = Path(f"/sys/dev/block/{os.major(device)}:{os.minor(device)}").resolve()
    if (tree / "partition").exists():
        raise ValueError("select the whole card, not a partition")
    # A mounted child partition means the entire card is busy.
    for line in Path("/proc/self/mountinfo").read_text().splitlines():
        mounted = Path("/sys/dev/block") / line.split()[2]
        node = mounted.resolve()
        if node == tree or tree in node.parents:
            raise ValueError("card has mounted partitions; unmount them first")
    for line in Path("/proc/swaps").read_text().splitlines()[1:]:
        swap = Path(line.split()[0])
        if swap.exists() and stat.S_ISBLK(swap.stat().st_mode):
            number = swap.stat().st_rdev
            node = Path(f"/sys/dev/block/{os.major(number)}:{os.minor(number)}").resolve()
            if node == tree or tree in node.parents:
                raise ValueError("card is used as swap")


# Рассчитывает изменения разметки без немедленной записи на устройство.
def plan(disk, total: int) -> tuple[list[tuple[int, bytes]], int, int]:
    if total % SECTOR:
        raise ValueError("device size is not a whole number of 512-byte sectors")
    mbr = bytearray(read_at(disk, 0, SECTOR))
    if mbr[510:] != b"\x55\xaa" or mbr[450] != 0xEE:
        raise ValueError("expected a protective GPT MBR")
    header = read_at(disk, SECTOR, SECTOR)
    check_header(header)
    current, old_backup, first, old_last = struct.unpack_from("<QQQQ", header, 24)
    entries_lba, count, size, checksum = struct.unpack_from("<QIII", header, 72)
    if current != 1 or entries_lba != 2 or count != 128 or size != 128 or first != 34:
        raise ValueError("unsupported GPT layout; expected this project's 512-byte-sector image")
    sectors = total // SECTOR
    if not old_last < old_backup < sectors or old_last != old_backup - 33:
        raise ValueError("GPT does not fit this card or uses an unsupported backup layout")
    entries = bytearray(read_at(disk, entries_lba * SECTOR, count * size))
    if zlib.crc32(entries) != checksum:
        raise ValueError("GPT entry checksum mismatch")
    backup = read_at(disk, old_backup * SECTOR, SECTOR)
    check_header(backup)
    if (backup[56:72] != header[56:72] or
            struct.unpack_from("<QQ", backup, 24) != (old_backup, 1) or
            backup[40:56] != header[40:56] or backup[80:92] != header[80:92]):
        raise ValueError("primary and backup GPT do not agree")
    backup_lba = struct.unpack_from("<Q", backup, 72)[0]
    if backup_lba != old_backup - 32 or read_at(disk, backup_lba * SECTOR, len(entries)) != entries:
        raise ValueError("backup GPT entries do not agree")
    occupied = []
    for offset in range(0, len(entries), size):
        entry = entries[offset:offset + size]
        if entry[:16] == bytes(16):
            continue
        name = entry[56:128].decode("utf-16-le").rstrip("\0")
        start, end = struct.unpack_from("<QQ", entry, 32)
        if not first <= start <= end <= old_last:
            raise ValueError("partition outside the usable GPT range")
        occupied.append((name, start, end, offset))
    if [item[0] for item in occupied] != EXPECTED:
        raise ValueError("this is not the expected watch card layout")
    for left, right in zip(occupied, occupied[1:]):
        if left[2] >= right[1]:
            raise ValueError("partitions overlap or are not ordered")
    _, start, old_end, offset = occupied[-1]
    if old_end != old_last:
        raise ValueError("userdata does not end at the old usable boundary")
    new_last = sectors - 34
    if new_last <= old_end:
        return [], old_end - start + 1, old_end - start + 1
    struct.pack_into("<Q", entries, offset + 40, new_last)
    checksum = zlib.crc32(entries)
    primary = make_header(header, 1, sectors - 1, new_last, 2, checksum)
    secondary = make_header(backup, sectors - 1, 1, new_last, sectors - 33, checksum)
    struct.pack_into("<I", mbr, 458, min(sectors - 1, 0xFFFFFFFF))
    # Backup table/header first; primary CRC is published after its entries.
    writes = [((sectors - 33) * SECTOR, bytes(entries)),
              ((sectors - 1) * SECTOR, secondary),
              (2 * SECTOR, bytes(entries)), (SECTOR, primary), (0, bytes(mbr))]
    return writes, old_end - start + 1, new_last - start + 1


# Разбирает параметры запуска и выполняет основной сценарий утилиты.
def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--device", required=True, type=Path)
    parser.add_argument("--apply", action="store_true", help="write only GPT/MBR metadata")
    parser.add_argument("--backup-dir", type=Path, default=Path("gpt-backups"))
    args = parser.parse_args()
    path = args.device.resolve(strict=True)
    with path.open("r+b" if args.apply else "rb", buffering=0) as disk:
        fcntl.flock(disk, fcntl.LOCK_EX | fcntl.LOCK_NB)
        info = os.fstat(disk.fileno())
        if not (stat.S_ISBLK(info.st_mode) or stat.S_ISREG(info.st_mode)):
            raise ValueError("expected a block device or raw image file")
        refuse_in_use(path, info.st_mode)
        if stat.S_ISBLK(info.st_mode):
            logical = struct.unpack("I", fcntl.ioctl(disk, 0x1268, bytes(4)))[0]  # BLKSSZGET
            if logical != SECTOR:
                raise ValueError("only 512-byte logical sector devices are supported")
            total = struct.unpack("Q", fcntl.ioctl(disk, 0x80081272, bytes(8)))[0]  # BLKGETSIZE64
        else:
            total = info.st_size
        writes, old, new = plan(disk, total)
        print(f"Device: {path} ({total / 1048576:.1f} MiB)")
        print(f"userdata: {old / 2048:.1f} -> {new / 2048:.1f} MiB")
        print("System, swap, partition starts, GUIDs and existing payloads remain unchanged.")
        if not writes:
            print("Already expanded to the device boundary.")
            return
        if not args.apply:
            print("Read-only plan. Add --apply to save GPT backup and apply.")
            return
        args.backup_dir.mkdir(parents=True, exist_ok=True)
        stamp = datetime.datetime.now(datetime.UTC).strftime("%Y%m%dT%H%M%S.%fZ")
        backup_path = args.backup_dir / f"{path.name}-{stamp}.json"
        snapshot = {"device": str(path), "size_bytes": total,
                    "segments": [{"offset": offset, "hex": read_at(disk, offset, len(data)).hex()}
                                 for offset, data in writes]}
        with backup_path.open("x", encoding="utf-8") as backup:
            json.dump(snapshot, backup)
            backup.flush()
            os.fsync(backup.fileno())
        directory = os.open(args.backup_dir, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
        print(f"GPT backup: {backup_path.resolve()}", flush=True)
        for offset, data in writes:
            disk.seek(offset)
            if disk.write(data) != len(data):
                raise OSError("short disk write; retain the GPT backup")
            os.fsync(disk.fileno())
        for offset, data in writes:
            if read_at(disk, offset, len(data)) != data:
                raise OSError("GPT read-back mismatch; retain the GPT backup")
        if stat.S_ISBLK(info.st_mode):
            try:
                fcntl.ioctl(disk, 0x125F)  # BLKRRPART, only after committed metadata
            except OSError:
                print("Partition table saved. Reinsert the card to refresh kernel partitions.")
        print("Expanded. No filesystem was formatted or resized.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError) as error:
        sys.exit(f"Refused/failed: {error}")
