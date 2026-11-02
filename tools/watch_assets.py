#!/usr/bin/env python3
"""Import a wallpaper or export a captured frame in a stopped watch disk image.

The image lock prevents concurrent writes by the block server or image builder.
Only the reserved userdata asset ranges are touched; this is not a filesystem.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import struct
import subprocess
import sys
import zlib

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from usb_block_server import locked_image
from inspect_android_sd import parse_primary_gpt, validate_backup_gpt

WIDTH, HEIGHT = 240, 280
LENGTH = WIDTH * HEIGHT * 2
SCREENSHOT_OFFSET = 1024 * 1024


def asset_header(magic: bytes, payload: bytes) -> bytes:
    header = bytearray(512)
    struct.pack_into('<8sHHII', header, 0, magic, WIDTH, HEIGHT,
                     len(payload), zlib.crc32(payload))
    return bytes(header)


def encode_wallpaper(path: Path) -> bytes:
    # ImageMagick handles image decoding, EXIF orientation and a centered crop.
    raw = subprocess.run(['magick', str(path.resolve()), '-auto-orient',
                          '-resize', '240x280^', '-gravity', 'center',
                          '-extent', '240x280', '-depth', '8', 'rgb:-'],
                         check=True, capture_output=True).stdout
    if len(raw) != WIDTH * HEIGHT * 3:
        raise ValueError('expected one image, not an animation or multiple frames')
    pixels = bytearray()
    for r, g, b in zip(raw[0::3], raw[1::3], raw[2::3]):
        pixels.extend(struct.pack('>H', ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)))
    return bytes(pixels)


def userdata_range(disk) -> tuple[int, int]:
    header, entries, parts = parse_primary_gpt(disk)
    validate_backup_gpt(disk, header, entries)
    first, last = parts['userdata']
    size = (last - first + 1) * 512
    if size < SCREENSHOT_OFFSET + 512 + LENGTH:
        raise ValueError('userdata is too small for reserved assets')
    return first * 512, size


def import_wallpaper(disk, payload: bytes) -> None:
    if len(payload) != LENGTH:
        raise ValueError('invalid wallpaper dimensions')
    base, _ = userdata_range(disk)
    disk.seek(base)
    old = disk.read(512)
    if any(old) and not old.startswith(b'WALLv1\0\0'):
        raise ValueError('userdata is not a watch asset area; refusing to overwrite it')
    # Publish the checksum header last so incomplete uploads cannot be rendered.
    disk.seek(base + 512)
    disk.write(payload)
    disk.flush()
    os.fsync(disk.fileno())
    disk.seek(base)
    disk.write(asset_header(b'WALLv1\0\0', payload))
    disk.flush()
    os.fsync(disk.fileno())


def export_screenshot(disk, output: Path) -> None:
    base, _ = userdata_range(disk)
    disk.seek(base + SCREENSHOT_OFFSET)
    header = disk.read(512)
    magic, width, height, length, checksum = struct.unpack_from('<8sHHII', header)
    if (magic, width, height, length) != (b'SHOTv1\0\0', WIDTH, HEIGHT, LENGTH):
        raise ValueError('no valid capture; run zadb.py shell screencap first')
    payload = disk.read(length)
    if len(payload) != length or zlib.crc32(payload) != checksum:
        raise ValueError('capture checksum mismatch')
    raw = bytearray()
    for (p,) in struct.iter_unpack('>H', payload):
        raw.extend((((p >> 11) & 31) * 255 // 31,
                    ((p >> 5) & 63) * 255 // 63, (p & 31) * 255 // 31))
    subprocess.run(['magick', '-size', '240x280', '-depth', '8', 'rgb:-',
                    str(output.resolve())], input=raw, check=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--image', type=Path, required=True)
    sub = parser.add_subparsers(dest='command', required=True)
    sub.add_parser('wallpaper').add_argument('source', type=Path)
    sub.add_parser('screenshot').add_argument('output', type=Path)
    args = parser.parse_args()
    payload = encode_wallpaper(args.source) if args.command == 'wallpaper' else None
    with locked_image(args.image, 'r+b' if payload is not None else 'rb') as disk:
        if payload is not None:
            import_wallpaper(disk, payload)
        else:
            export_screenshot(disk, args.output)
    print('OK')


if __name__ == '__main__':
    main()
