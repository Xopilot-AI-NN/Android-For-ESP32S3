#!/usr/bin/env python3
"""Build a sparse Android-like microSD image for Zephyr Watch.

The generated card uses a real GPT and A/B physical boot partitions:
  misc, metadata, boot_a/b, init_boot_a/b, vendor_boot_a/b, vbmeta_a/b,
  super, userdata.

boot/init_boot use Android boot image header v4 (ANDROID!).
vendor_boot uses Android vendor boot header v4 (VNDRBOOT).
vbmeta uses the real AVB0 container and AVB hash descriptors with
algorithm NONE for development builds. The bootloader still verifies
SHA-256 descriptors; signature verification is intentionally deferred.
"""
from __future__ import annotations

import argparse
import hashlib
import os
import struct
import uuid
import zlib
from pathlib import Path

SECTOR = 512
ALIGN_LBA = 2048  # 1 MiB
GPT_ENTRIES = 128
GPT_ENTRY_SIZE = 128
GPT_ENTRIES_SECTORS = GPT_ENTRIES * GPT_ENTRY_SIZE // SECTOR
BASIC_DATA_GUID = uuid.UUID("EBD0A0A2-B9E5-4433-87C0-68B6B72699C7")
DISK_NAMESPACE = uuid.UUID("adf41d27-a230-4e4e-9af8-6ecb2c31cf65")
BOOT_CTRL_MAGIC = 0x42414342


def align_up(v: int, a: int) -> int:
    return (v + a - 1) // a * a


def guid_le(g: uuid.UUID) -> bytes:
    return g.bytes_le


def cpio_newc(files: dict[str, bytes]) -> bytes:
    out = bytearray()
    ino = 1
    for name, data in files.items():
        name_b = name.encode() + b"\0"
        fields = [
            ino, 0o100755, 0, 0, 1, 0, len(data), 0, 0, 0, 0, len(name_b), 0
        ]
        out += b"070701" + b"".join(f"{x:08x}".encode() for x in fields)
        out += name_b
        out += b"\0" * ((-len(out)) % 4)
        out += data
        out += b"\0" * ((-len(out)) % 4)
        ino += 1
    trailer = b"TRAILER!!!\0"
    fields = [ino, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, len(trailer), 0]
    out += b"070701" + b"".join(f"{x:08x}".encode() for x in fields)
    out += trailer
    out += b"\0" * ((-len(out)) % 4)
    return bytes(out)


def android_boot_v4(ramdisk: bytes, version: str) -> bytes:
    header = bytearray(4096)
    header[0:8] = b"ANDROID!"
    struct.pack_into("<I", header, 8, 0)  # kernel_size: Rust runtime replaces Linux kernel
    struct.pack_into("<I", header, 12, len(ramdisk))
    struct.pack_into("<I", header, 16, 0)  # os_version
    struct.pack_into("<I", header, 20, 1584)
    struct.pack_into("<I", header, 40, 4)
    cmdline = f"androidboot.hardware=esp32s3 androidboot.zephyr.version={version}".encode()
    header[44:44 + min(len(cmdline), 1535)] = cmdline[:1535]
    struct.pack_into("<I", header, 1580, 0)  # signature_size
    image = bytes(header) + ramdisk
    return image + b"\0" * ((-len(image)) % 4096)


def vendor_boot_v4(version: str) -> bytes:
    header = bytearray(4096)
    header[0:8] = b"VNDRBOOT"
    struct.pack_into("<I", header, 8, 4)
    struct.pack_into("<I", header, 12, 4096)
    struct.pack_into("<I", header, 16, 0)  # kernel_addr
    struct.pack_into("<I", header, 20, 0)  # ramdisk_addr
    struct.pack_into("<I", header, 24, 0)  # vendor_ramdisk_size
    cmdline = f"androidboot.hardware=esp32s3 androidboot.product=zephyr_watch androidboot.zephyr.version={version}".encode()
    header[28:28 + min(len(cmdline), 2047)] = cmdline[:2047]
    struct.pack_into("<I", header, 2076, 0)  # tags_addr
    header[2080:2096] = b"zephyr-watch\0\0\0\0"
    struct.pack_into("<I", header, 2096, 2128)
    struct.pack_into("<I", header, 2100, 0)  # dtb_size
    struct.pack_into("<Q", header, 2104, 0)  # dtb_addr
    struct.pack_into("<I", header, 2112, 0)  # vendor_ramdisk_table_size
    struct.pack_into("<I", header, 2116, 0)  # entries
    struct.pack_into("<I", header, 2120, 108)  # standard v4 table entry size
    struct.pack_into("<I", header, 2124, 0)  # bootconfig_size
    return bytes(header)


def avb_hash_descriptor(partition_name: str, image: bytes, salt: bytes) -> bytes:
    name = partition_name.encode()
    digest = hashlib.sha256(salt + image).digest()
    payload = bytearray()
    payload += struct.pack(">Q", len(image))
    payload += b"sha256\0" + b"\0" * (32 - 7)
    payload += struct.pack(">III", len(name), len(salt), len(digest))
    payload += struct.pack(">I", 0)  # A/B suffix applies
    payload += bytes(60)
    payload += name + salt + digest
    payload += bytes((-len(payload)) % 8)
    return struct.pack(">QQ", 2, len(payload)) + payload


def vbmeta_image(images: dict[str, bytes], rollback_index: int = 0) -> bytes:
    descriptors = bytearray()
    for name, image in images.items():
        salt = hashlib.sha256(("zephyr:" + name).encode()).digest()
        descriptors += avb_hash_descriptor(name, image, salt)
    aux_size = align_up(len(descriptors), 64)
    aux = bytes(descriptors) + bytes(aux_size - len(descriptors))
    header = bytearray(256)
    header[0:4] = b"AVB0"
    struct.pack_into(">I", header, 4, 1)
    struct.pack_into(">I", header, 8, 0)
    struct.pack_into(">Q", header, 12, 0)  # auth block
    struct.pack_into(">Q", header, 20, aux_size)
    struct.pack_into(">I", header, 28, 0)  # AVB_ALGORITHM_TYPE_NONE
    struct.pack_into(">Q", header, 96, 0)  # descriptors offset in aux
    struct.pack_into(">Q", header, 104, len(descriptors))
    struct.pack_into(">Q", header, 112, rollback_index)
    struct.pack_into(">I", header, 120, 0)
    release = b"avbtool-compatible zephyr dev"
    header[128:128 + len(release)] = release
    return bytes(header) + aux


def boot_control() -> bytes:
    raw = bytearray(32)
    raw[0:3] = b"_a\0"
    struct.pack_into("<I", raw, 4, BOOT_CTRL_MAGIC)
    raw[8] = 1
    raw[9] = 2  # nb_slot
    raw[12] = 15 | (7 << 4)
    raw[14] = 14 | (7 << 4)
    struct.pack_into("<I", raw, 28, zlib.crc32(raw[:28]) & 0xFFFFFFFF)
    return bytes(raw)


def protective_mbr(total_lba: int) -> bytes:
    mbr = bytearray(512)
    count = min(total_lba - 1, 0xFFFFFFFF)
    entry = bytearray(16)
    entry[4] = 0xEE
    struct.pack_into("<I", entry, 8, 1)
    struct.pack_into("<I", entry, 12, count)
    mbr[446:462] = entry
    mbr[510:512] = b"\x55\xaa"
    return bytes(mbr)


def partition_entry(name: str, first: int, last: int, disk_guid: uuid.UUID) -> bytes:
    e = bytearray(128)
    e[0:16] = guid_le(BASIC_DATA_GUID)
    e[16:32] = guid_le(uuid.uuid5(disk_guid, name))
    struct.pack_into("<Q", e, 32, first)
    struct.pack_into("<Q", e, 40, last)
    name_utf16 = name.encode("utf-16le")[:72]
    e[56:56 + len(name_utf16)] = name_utf16
    return bytes(e)


def gpt_headers(total_lba: int, parts: list[tuple[str, int, int]], disk_guid: uuid.UUID):
    entries = bytearray(GPT_ENTRIES * GPT_ENTRY_SIZE)
    for i, (name, first, last) in enumerate(parts):
        entries[i * 128:(i + 1) * 128] = partition_entry(name, first, last, disk_guid)
    entries_crc = zlib.crc32(entries) & 0xFFFFFFFF
    first_usable = 2 + GPT_ENTRIES_SECTORS
    last_usable = total_lba - GPT_ENTRIES_SECTORS - 2

    def hdr(current, backup, entries_lba):
        h = bytearray(512)
        h[0:8] = b"EFI PART"
        struct.pack_into("<I", h, 8, 0x00010000)
        struct.pack_into("<I", h, 12, 92)
        struct.pack_into("<Q", h, 24, current)
        struct.pack_into("<Q", h, 32, backup)
        struct.pack_into("<Q", h, 40, first_usable)
        struct.pack_into("<Q", h, 48, last_usable)
        h[56:72] = guid_le(disk_guid)
        struct.pack_into("<Q", h, 72, entries_lba)
        struct.pack_into("<I", h, 80, GPT_ENTRIES)
        struct.pack_into("<I", h, 84, GPT_ENTRY_SIZE)
        struct.pack_into("<I", h, 88, entries_crc)
        struct.pack_into("<I", h, 16, zlib.crc32(h[:92]) & 0xFFFFFFFF)
        return bytes(h)

    primary = hdr(1, total_lba - 1, 2)
    backup_entries_lba = total_lba - GPT_ENTRIES_SECTORS - 1
    backup = hdr(total_lba - 1, 1, backup_entries_lba)
    return primary, bytes(entries), backup, backup_entries_lba


def mib(n: int) -> int:
    return n * 1024 * 1024


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--output", default="out/zephyr-watch-sd.img")
    ap.add_argument("--size-mib", type=int, default=512)
    ap.add_argument("--version", default="0.2.0")
    ap.add_argument("--init", default="sdcard/android/init.rhai")
    args = ap.parse_args()
    if args.size_mib < 256:
        raise SystemExit("SD image must be at least 256 MiB")

    init_script = Path(args.init).read_bytes()
    generic_ramdisk = cpio_newc({"init.rhai": init_script})
    boot = android_boot_v4(b"", args.version)
    init_boot = android_boot_v4(generic_ramdisk, args.version)
    vendor_boot = vendor_boot_v4(args.version)
    vbmeta = vbmeta_image({"boot": boot, "init_boot": init_boot, "vendor_boot": vendor_boot})

    layout_mib = [
        ("misc", 4), ("metadata", 16),
        ("boot_a", 8), ("boot_b", 8),
        ("init_boot_a", 8), ("init_boot_b", 8),
        ("vendor_boot_a", 16), ("vendor_boot_b", 16),
        ("vbmeta_a", 1), ("vbmeta_b", 1),
        ("super", 128),
    ]
    total_bytes = mib(args.size_mib)
    total_lba = total_bytes // SECTOR
    cursor = ALIGN_LBA
    parts: list[tuple[str, int, int]] = []
    for name, size_mib in layout_mib:
        blocks = mib(size_mib) // SECTOR
        cursor = align_up(cursor, ALIGN_LBA)
        parts.append((name, cursor, cursor + blocks - 1))
        cursor += blocks
    cursor = align_up(cursor, ALIGN_LBA)
    backup_start = total_lba - GPT_ENTRIES_SECTORS - 1
    if cursor + ALIGN_LBA >= backup_start:
        raise SystemExit("image too small for partition layout")
    parts.append(("userdata", cursor, backup_start - 1))

    disk_guid = uuid.uuid5(DISK_NAMESPACE, f"zephyr-watch-{args.version}")
    primary, entries, backup, backup_entries_lba = gpt_headers(total_lba, parts, disk_guid)
    out = Path(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("wb") as f:
        f.truncate(total_bytes)
        f.seek(0); f.write(protective_mbr(total_lba))
        f.seek(SECTOR); f.write(primary)
        f.seek(2 * SECTOR); f.write(entries)
        f.seek(backup_entries_lba * SECTOR); f.write(entries)
        f.seek((total_lba - 1) * SECTOR); f.write(backup)

        by_name = {name: (first, last) for name, first, last in parts}
        def put(name: str, blob: bytes, offset: int = 0):
            first, last = by_name[name]
            cap = (last - first + 1) * SECTOR
            if offset + len(blob) > cap:
                raise SystemExit(f"{name}: image exceeds partition")
            f.seek(first * SECTOR + offset)
            f.write(blob)

        put("misc", boot_control(), 2048)
        for suffix in ("a", "b"):
            put(f"boot_{suffix}", boot)
            put(f"init_boot_{suffix}", init_boot)
            put(f"vendor_boot_{suffix}", vendor_boot)
            put(f"vbmeta_{suffix}", vbmeta)

    print(f"created {out} ({args.size_mib} MiB sparse-capable raw GPT image)")
    for name, first, last in parts:
        print(f"{name:16s} LBA {first:8d}..{last:8d}  {(last-first+1)*SECTOR//(1024*1024):4d} MiB")
    print("AVB: real AVB0/hash descriptors, algorithm NONE (development/orange state)")


if __name__ == "__main__":
    main()
