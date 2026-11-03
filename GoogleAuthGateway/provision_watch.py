#!/usr/bin/env python3
"""Provision a stopped private disk image; never send device credentials over Wi-Fi ADB."""
import argparse
import json
import os
from pathlib import Path
import re
import struct
import time
from urllib.parse import urlsplit
import zlib
try:
    import fcntl
except ImportError:
    fcntl = None

PAGE = 4096
TAG = 0x47535731
SLOTS = (76, 77)


def read_exact(image, offset, length):
    """Reject truncated headers rather than guessing a partition boundary."""
    image.seek(offset)
    data = image.read(length)
    if len(data) != length:
        raise ValueError("truncated image")
    return data


def swap_partition(image):
    """Verify GPT header/table CRCs before locating the existing swap partition."""
    header = bytearray(read_exact(image, 512, 512))
    if header[:8] != b"EFI PART":
        raise ValueError("not a GPT image")
    size, checksum = struct.unpack_from("<II", header, 12)
    if not 92 <= size <= 512:
        raise ValueError("invalid GPT header")
    header[16:20] = b"\0" * 4
    if zlib.crc32(header[:size]) != checksum:
        raise ValueError("GPT header CRC mismatch")
    lba, count, width, checksum = struct.unpack_from("<QIII", header, 72)
    if not 1 <= count <= 1024 or not 128 <= width <= 1024:
        raise ValueError("unsupported GPT table")
    table = read_exact(image, lba * 512, count * width)
    if zlib.crc32(table) != checksum:
        raise ValueError("GPT table CRC mismatch")
    image.seek(0, 2)
    disk_sectors = image.tell() // 512
    first, last = struct.unpack_from('<QQ', header, 40)
    if first < lba + (count * width + 511) // 512 or last >= disk_sectors - 1 or first > last:
        raise ValueError('invalid GPT usable range')
    partitions = []
    swaps = []
    for index in range(count):
        entry = table[index * width:(index + 1) * width]
        if not any(entry[:16]):
            continue
        name = entry[56:128].decode("utf-16-le").split("\0", 1)[0]
        start, end = struct.unpack_from('<QQ', entry, 32)
        if start < first or end > last or end < start or any(start <= old_end and end >= old_start for old_start, old_end in partitions):
            raise ValueError('overlapping or out-of-range GPT partition')
        partitions.append((start, end))
        if name == 'swap':
            if (end - start + 1) * 512 < 81 * PAGE:
                raise ValueError('swap partition is too small')
            swaps.append(start * 512)
    if len(swaps) == 1:
        return swaps[0]
    raise ValueError("existing swap partition not found; build a system image first")


def record(data):
    """Use exactly the ZPager v1 header/CRC format consumed by Firmware."""
    return struct.pack("<4sIHHI", b"ZPG1", TAG, len(data), 1, zlib.crc32(data)) + data + bytes(PAGE - 16 - len(data))


def generation(page):
    if not any(page):
        return 0
    magic, tag, length, version, crc = struct.unpack_from("<4sIHHI", page)
    if magic != b"ZPG1" or tag != TAG:
        raise ValueError("reserved configuration slot contains foreign data")
    # A torn write belonging to us may be replaced; the other copy stays valid.
    if version != 1 or length > PAGE - 16 or zlib.crc32(page[16:16 + length]) != crc:
        return 0
    try:
        value = json.loads(page[16:16 + length])
        return int(value["generation"])
    except (ValueError, KeyError, TypeError):
        return 0


def provision(image_path, device_path, ca_path, base_url):
    """Publish one verified alternate configuration page, keeping all other bytes intact."""
    uri = urlsplit(base_url)
    if uri.scheme != "https" or not uri.hostname or uri.username or uri.password or uri.path or uri.query or uri.fragment or len(base_url) > 150:
        raise ValueError("a bare HTTPS Gateway origin is required")
    if not re.fullmatch(r"[A-Za-z0-9.-]{1,120}", uri.hostname) or not 1 <= (uri.port or 443) <= 65535:
        raise ValueError("unsupported Gateway hostname/port")
    device = json.loads(Path(device_path).read_text())
    if any(not isinstance(device.get(key), str) or not re.fullmatch(r"[A-Za-z0-9_-]{16,128}", device[key]) for key in ("device_id", "device_credential")):
        raise ValueError("expected private device JSON from register")
    ca = Path(ca_path).read_text()
    if len(ca.encode()) > 3000 or "\0" in ca or "-----BEGIN CERTIFICATE-----" not in ca or "-----END CERTIFICATE-----" not in ca:
        raise ValueError("provide one PEM trust anchor, at most 3000 bytes")
    # The file lock is the same sidecar flock used by usb_block_server.py.
    # Refuse unsupported locking rather than race the running PC block server.
    if fcntl is None:
        raise ValueError("safe image provisioning requires Linux/macOS/Termux file locking")
    image_path = Path(image_path).resolve()
    with image_path.with_suffix(image_path.suffix + ".lock").open("a+b") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise ValueError("image is in use; stop the block server first") from error
        with image_path.open("r+b") as image:
            if os.name == "posix":
                os.fchmod(image.fileno(), 0o600)
            start = swap_partition(image)
            offsets = [start + (slot + 1) * PAGE for slot in SLOTS]
            generations = [generation(read_exact(image, at, PAGE)) for at in offsets]
            value = {"version": 1, "generation": max(time.time_ns(), max(generations) + 1),
                     "base_url": base_url, "device_id": device["device_id"],
                     "device_credential": device["device_credential"], "ca_pem": ca}
            payload = json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode()
            if len(payload) > PAGE - 16:
                raise ValueError("configuration exceeds one ZPager page")
            destination = offsets[generations.index(min(generations))]
            page = record(payload)
            image.seek(destination)
            image.write(page)
            image.flush()
            os.fsync(image.fileno())
            if read_exact(image, destination, PAGE) != page:
                raise ValueError("configuration readback mismatch")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("device_json", type=Path)
    parser.add_argument("ca_pem", type=Path)
    parser.add_argument("image", type=Path)
    parser.add_argument("--base-url", required=True)
    args = parser.parse_args()
    try:
        provision(args.image, args.device_json, args.ca_pem, args.base_url.rstrip("/"))
    except (ValueError, OSError) as error:
        parser.exit(1, str(error) + "\n")
    print("Watch configuration written and verified. Keep this disk image private; restart the watch.")


if __name__ == "__main__":
    main()
