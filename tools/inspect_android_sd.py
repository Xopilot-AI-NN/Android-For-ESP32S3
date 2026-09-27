#!/usr/bin/env python3
"""Offline integrity checker for Zephyr Watch Android-style microSD images."""
from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from pathlib import Path

SECTOR = 512
BOOT_HEADER_V4_SIZE = 1584
VENDOR_BOOT_HEADER_V4_SIZE = 2128


def u32le(b, o): return struct.unpack_from('<I', b, o)[0]
def u64le(b, o): return struct.unpack_from('<Q', b, o)[0]
def u32be(b, o): return struct.unpack_from('>I', b, o)[0]
def u64be(b, o): return struct.unpack_from('>Q', b, o)[0]


def validate_header_crc(header: bytes) -> None:
    assert header[:8] == b'EFI PART', 'GPT magic'
    hs = u32le(header, 12)
    assert 92 <= hs <= SECTOR, 'GPT header size'
    expected = u32le(header, 16)
    copy = bytearray(header[:hs])
    copy[16:20] = b'\0' * 4
    assert zlib.crc32(copy) & 0xFFFFFFFF == expected, 'GPT header CRC'


def parse_primary_gpt(f):
    f.seek(0)
    mbr = f.read(SECTOR)
    assert mbr[510:512] == b'\x55\xaa', 'protective MBR signature'
    assert mbr[446 + 4] == 0xEE, 'protective MBR type'

    f.seek(SECTOR)
    h = f.read(SECTOR)
    validate_header_crc(h)
    assert u64le(h, 24) == 1, 'primary GPT current_lba'
    elba, n, es, ecrc = u64le(h, 72), u32le(h, 80), u32le(h, 84), u32le(h, 88)
    f.seek(elba * SECTOR)
    entries = f.read(n * es)
    assert len(entries) == n * es, 'short GPT entry array'
    assert zlib.crc32(entries) & 0xFFFFFFFF == ecrc, 'GPT entry array CRC'

    parts = {}
    for i in range(n):
        e = entries[i * es:(i + 1) * es]
        if e[:16] == b'\0' * 16:
            continue
        first, last = u64le(e, 32), u64le(e, 40)
        assert first and last >= first, f'bad GPT range entry {i}'
        name = e[56:128].decode('utf-16le').split('\0', 1)[0]
        parts[name] = (first, last)
    return h, entries, parts


def validate_backup_gpt(f, primary: bytes, entries: bytes) -> None:
    backup_lba = u64le(primary, 32)
    f.seek(backup_lba * SECTOR)
    backup = f.read(SECTOR)
    validate_header_crc(backup)
    assert u64le(backup, 24) == backup_lba, 'backup GPT current_lba'
    assert u64le(backup, 32) == 1, 'backup GPT alternate_lba'
    assert backup[56:72] == primary[56:72], 'disk GUID mismatch'
    elba = u64le(backup, 72)
    n, es, ecrc = u32le(backup, 80), u32le(backup, 84), u32le(backup, 88)
    f.seek(elba * SECTOR)
    backup_entries = f.read(n * es)
    assert backup_entries == entries, 'backup GPT entries differ'
    assert zlib.crc32(backup_entries) & 0xFFFFFFFF == ecrc, 'backup GPT entry CRC'


def read_partition(f, parts, name, used_size=None):
    first, last = parts[name]
    cap = (last - first + 1) * SECTOR
    f.seek(first * SECTOR)
    return f.read(cap if used_size is None else min(used_size, cap))


def validate_boot_v4(blob: bytes, expect_ramdisk: bool) -> tuple[int, int]:
    assert blob[:8] == b'ANDROID!', 'boot magic'
    kernel_size = u32le(blob, 8)
    ramdisk_size = u32le(blob, 12)
    assert u32le(blob, 20) == BOOT_HEADER_V4_SIZE, 'boot header_size'
    assert u32le(blob, 40) == 4, 'boot header_version'
    assert kernel_size == 0, 'ESP32 boot image must not carry Linux kernel'
    assert (ramdisk_size > 0) == expect_ramdisk, 'unexpected ramdisk presence'
    return kernel_size, ramdisk_size


def cpio_has_init(blob: bytes, base: int, size: int) -> bool:
    cursor = 0
    while cursor + 110 <= size:
        h = blob[base + cursor:base + cursor + 110]
        assert h[:6] in (b'070701', b'070702'), 'CPIO magic'
        filesize = int(h[54:62], 16)
        namesize = int(h[94:102], 16)
        name_off = base + cursor + 110
        name_raw = blob[name_off:name_off + namesize]
        name = name_raw.split(b'\0', 1)[0].decode()
        data_off_rel = (cursor + 110 + namesize + 3) & ~3
        if name == 'TRAILER!!!':
            return False
        if name.lstrip('./') == 'init.rhai' and filesize > 0:
            return True
        cursor = (data_off_rel + filesize + 3) & ~3
    return False


def validate_vendor_boot_v4(blob: bytes) -> None:
    assert blob[:8] == b'VNDRBOOT', 'vendor_boot magic'
    assert u32le(blob, 8) == 4, 'vendor_boot version'
    assert u32le(blob, 12) == 4096, 'vendor_boot page size'
    assert u32le(blob, 2096) == VENDOR_BOOT_HEADER_V4_SIZE, 'vendor_boot header size'
    assert u32le(blob, 2120) == 108, 'vendor ramdisk table entry size'


def validate_vbmeta(v: bytes, blobs: dict[str, bytes]) -> None:
    assert v[:4] == b'AVB0', 'vbmeta magic'
    assert u32be(v, 4) == 1, 'AVB major version'
    assert u32be(v, 28) == 0, 'development vbmeta must be algorithm NONE'
    assert u64be(v, 12) == 0, 'unexpected auth block in development vbmeta'
    aux = 256 + u64be(v, 12)
    cur = aux + u64be(v, 96)
    end = cur + u64be(v, 104)
    seen = set()
    while cur < end:
        tag, following = u64be(v, cur), u64be(v, cur + 8)
        total = 16 + following
        assert cur + total <= end, 'descriptor exceeds descriptor block'
        if tag == 2:
            image_size = u64be(v, cur + 16)
            alg = v[cur + 24:cur + 56].split(b'\0', 1)[0]
            assert alg == b'sha256', 'unsupported AVB hash'
            nl, sl, dl = u32be(v, cur + 56), u32be(v, cur + 60), u32be(v, cur + 64)
            assert dl == 32, 'bad SHA-256 digest length'
            base = cur + 132
            name = v[base:base + nl].decode()
            salt = v[base + nl:base + nl + sl]
            digest = v[base + nl + sl:base + nl + sl + dl]
            assert name in blobs, f'unknown AVB descriptor {name}'
            assert image_size <= len(blobs[name]), f'{name}: AVB image_size out of range'
            actual = hashlib.sha256(salt + blobs[name][:image_size]).digest()
            assert actual == digest, f'{name}: AVB SHA-256 mismatch'
            seen.add(name)
        cur += total
    assert seen == {'boot', 'init_boot', 'vendor_boot'}, f'missing AVB descriptors: {seen}'


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('image')
    a = ap.parse_args()
    p = Path(a.image)
    with p.open('rb') as f:
        primary, entries, parts = parse_primary_gpt(f)
        validate_backup_gpt(f, primary, entries)
        required = [
            'misc', 'metadata', 'boot_a', 'boot_b', 'init_boot_a', 'init_boot_b',
            'vendor_boot_a', 'vendor_boot_b', 'vbmeta_a', 'vbmeta_b', 'super', 'swap', 'userdata'
        ]
        assert all(x in parts for x in required), 'required partition missing'

        first, _ = parts['misc']
        f.seek(first * SECTOR + 2048)
        ctrl = f.read(32)
        assert u32le(ctrl, 4) == 0x42414342, 'boot_control magic'
        assert ctrl[8] == 1 and (ctrl[9] & 7) == 2, 'boot_control version/slot count'
        assert zlib.crc32(ctrl[:28]) & 0xFFFFFFFF == u32le(ctrl, 28), 'boot_control CRC'

        for slot in ('a', 'b'):
            blobs = {}
            for name in ('boot', 'init_boot', 'vendor_boot', 'vbmeta'):
                blobs[name] = read_partition(f, parts, f'{name}_{slot}')
            validate_boot_v4(blobs['boot'], expect_ramdisk=False)
            _, init_ramdisk_size = validate_boot_v4(blobs['init_boot'], expect_ramdisk=True)
            assert cpio_has_init(blobs['init_boot'], 4096, init_ramdisk_size), 'init.rhai missing from init_boot CPIO'
            validate_vendor_boot_v4(blobs['vendor_boot'])
            validate_vbmeta(blobs['vbmeta'], blobs)

    print('OK: protective MBR, primary+backup GPT CRCs, A/B misc metadata, Android v4 images, CPIO init.rhai and AVB SHA-256 descriptors verified')


if __name__ == '__main__':
    main()
