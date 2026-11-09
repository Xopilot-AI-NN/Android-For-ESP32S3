#!/usr/bin/env python3
"""Inspect and update a partition in a stopped PC-block GPT image.

This tool operates on the host image, not on internal ESP32 SPI flash.
The shared image lock prevents a race with the block server or firmware builder.
"""
from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from usb_block_server import locked_image
from inspect_android_sd import parse_primary_gpt, validate_backup_gpt

# User documents, swap, PIN and A/B control data are never firmware payloads.
FLASHABLE = {f"{name}_{slot}" for name in ("boot", "init_boot", "vendor_boot", "vbmeta")
             for slot in ("a", "b")} | {"super"}
CHUNK = 64 * 1024


def partitions(disk) -> dict[str, tuple[int, int]]:
    """Validate both GPT copies and every range before exposing byte offsets."""
    header, entries, parts = parse_primary_gpt(disk)
    validate_backup_gpt(disk, header, entries)
    size = os.fstat(disk.fileno()).st_size
    ranges = sorted((first, last) for first, last in parts.values())
    for index, (first, last) in enumerate(ranges):
        if first < 34 or last < first or (last + 34) * 512 > size:
            raise ValueError("GPT partition is outside the image's usable range")
        if index and first <= ranges[index - 1][1]:
            raise ValueError("GPT partitions overlap")
    return {name: (first * 512, (last - first + 1) * 512)
            for name, (first, last) in parts.items()}


def flash_partition(disk, name: str, source: Path) -> str:
    """Write only the selected firmware payload, fsync and verify its SHA-256.

    The partition tail is deliberately preserved. This operation is not atomic:
    a failed write requires retrying the same image before attempting boot.
    """
    if name not in FLASHABLE:
        raise ValueError("not a firmware partition; userdata/misc/swap are protected")
    base, capacity = partitions(disk)[name]
    with source.open("rb") as payload:
        size = os.fstat(payload.fileno()).st_size
        if size <= 0 or size > capacity:
            raise ValueError("payload is empty or exceeds the partition")
        if os.path.samestat(os.fstat(disk.fileno()), os.fstat(payload.fileno())):
            raise ValueError("source and disk are the same file")
        expected = hashlib.sha256()
        disk.seek(base)
        remaining = size
        while remaining:
            chunk = payload.read(min(CHUNK, remaining))
            if not chunk:
                raise ValueError("source changed during upload")
            if disk.write(chunk) != len(chunk):
                raise OSError("short partition write")
            expected.update(chunk)
            remaining -= len(chunk)
        disk.flush()
        os.fsync(disk.fileno())
    disk.seek(base)
    actual = hashlib.sha256()
    remaining = size
    while remaining:
        chunk = disk.read(min(CHUNK, remaining))
        if not chunk:
            raise OSError("short verification read")
        actual.update(chunk)
        remaining -= len(chunk)
    if actual.digest() != expected.digest():
        raise OSError("partition readback SHA-256 mismatch")
    return actual.hexdigest()


def main() -> None:
    """Require an explicit image, operation and partition; never select a disk automatically."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", type=Path, required=True)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("list")
    flash = commands.add_parser("flash")
    flash.add_argument("partition", choices=sorted(FLASHABLE))
    flash.add_argument("source", type=Path)
    args = parser.parse_args()
    with locked_image(args.image.resolve(), "r+b" if args.command == "flash" else "rb") as disk:
        if args.command == "list":
            for name, (offset, size) in partitions(disk).items():
                print(f"{name:20} offset={offset:#x} size={size}")
        else:
            print(flash_partition(disk, args.partition, args.source))


if __name__ == "__main__":
    main()
