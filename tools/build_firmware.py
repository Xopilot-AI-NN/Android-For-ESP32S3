#!/usr/bin/env python3
"""Build AOSP Wear OS Android 17 QPR1 firmware for ESP32-S3.

Outputs Android-style boot images, AOSP liblp `super.img`, logical partition
images, and a complete raw GPT disk image for microSD/USB-PC block boot.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import struct
import tempfile
import uuid
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SECTOR = 512
MIB = 1024 * 1024
ALIGN = MIB
ALIGN_LBA = ALIGN // SECTOR
GPT_ENTRIES = 128
GPT_ENTRY_SIZE = 128
GPT_ENTRIES_SECTORS = GPT_ENTRIES * GPT_ENTRY_SIZE // SECTOR
BASIC_DATA_GUID = uuid.UUID("EBD0A0A2-B9E5-4433-87C0-68B6B72699C7")
DISK_NAMESPACE = uuid.UUID("5fb6384f-e56b-4b7b-bfea-77c29f89971c")
BOOT_CTRL_MAGIC = 0x42414342

# Android liblp constants from system/core/fs_mgr/liblp/metadata_format.h.
LP_GEOMETRY_MAGIC = 0x616C4467
LP_HEADER_MAGIC = 0x414C5030
LP_GEOMETRY_SIZE = 4096
LP_RESERVED_BYTES = 4096
LP_METADATA_MAX = 65536
LP_METADATA_SLOTS = 2
LP_HEADER_SIZE = 128
LP_PARTITION_ATTR_READONLY = 1

SUPER_SIZE = 128 * MIB
LOGICAL_LAYOUT = [
    ("system_a", 16, 0), ("system_b", 16, 1),
    ("system_ext_a", 4, 0), ("system_ext_b", 4, 1),
    ("vendor_a", 8, 0), ("vendor_b", 8, 1),
    ("product_a", 8, 0), ("product_b", 8, 1),
    ("odm_a", 4, 0), ("odm_b", 4, 1),
]


def align_up(v: int, a: int) -> int:
    return (v + a - 1) // a * a


def cpio_newc(files: dict[str, bytes]) -> bytes:
    out = bytearray()
    ino = 1
    for name in sorted(files):
        data = files[name]
        name_b = name.encode("utf-8") + b"\0"
        fields = [ino, 0o100644, 0, 0, 1, 0, len(data), 0, 0, 0, 0, len(name_b), 0]
        out += b"070701" + b"".join(f"{x:08x}".encode() for x in fields)
        out += name_b
        out += bytes((-len(out)) % 4)
        out += data
        out += bytes((-len(out)) % 4)
        ino += 1
    trailer = b"TRAILER!!!\0"
    fields = [ino, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, len(trailer), 0]
    out += b"070701" + b"".join(f"{x:08x}".encode() for x in fields)
    out += trailer
    out += bytes((-len(out)) % 4)
    return bytes(out)


def tree_files(root: Path) -> dict[str, bytes]:
    result: dict[str, bytes] = {}
    for p in sorted(root.rglob("*")):
        if p.is_file():
            result[p.relative_to(root).as_posix()] = p.read_bytes()
    return result


def bundle_framework(system_root: Path) -> bytes:
    chunks = []
    for p in sorted((system_root / "framework" / "src").glob("*.rhai")):
        chunks.append(f"\n// ===== {p.name} =====\n".encode())
        chunks.append(p.read_bytes())
        chunks.append(b"\n")
    return b"".join(chunks)


def prepare_partition_tree(partition: str, staging: Path) -> Path:
    src = ROOT / partition
    dst = staging / partition
    shutil.copytree(src, dst)
    if partition == "system":
        bundle = bundle_framework(src)
        (dst / "framework" / "aosp-wear-framework.rhai").write_bytes(bundle)
        shutil.rmtree(dst / "framework" / "src")
    return dst


def android_boot_v4(ramdisk: bytes, version: str) -> bytes:
    header = bytearray(4096)
    header[0:8] = b"ANDROID!"
    struct.pack_into("<I", header, 8, 0)
    struct.pack_into("<I", header, 12, len(ramdisk))
    struct.pack_into("<I", header, 16, 0)
    struct.pack_into("<I", header, 20, 1584)
    struct.pack_into("<I", header, 40, 4)
    cmdline = (
        f"androidboot.hardware=esp32s3 androidboot.product=aosp_wear "
        f"androidboot.slot_suffix=_a androidboot.aosp_wear.version={version}"
    ).encode()
    header[44:44 + min(len(cmdline), 1535)] = cmdline[:1535]
    struct.pack_into("<I", header, 1580, 0)
    image = bytes(header) + ramdisk
    return image + bytes((-len(image)) % 4096)


def vendor_boot_v4(version: str) -> bytes:
    header = bytearray(4096)
    header[0:8] = b"VNDRBOOT"
    struct.pack_into("<I", header, 8, 4)
    struct.pack_into("<I", header, 12, 4096)
    cmdline = f"androidboot.hardware=esp32s3 androidboot.aosp_wear.version={version}".encode()
    header[28:28 + min(len(cmdline), 2047)] = cmdline[:2047]
    header[2080:2096] = b"aosp-wear\0\0\0\0\0\0\0"
    struct.pack_into("<I", header, 2096, 2128)
    struct.pack_into("<I", header, 2120, 108)
    return bytes(header)


def avb_hash_descriptor(partition_name: str, image: bytes, salt: bytes) -> bytes:
    name = partition_name.encode()
    digest = hashlib.sha256(salt + image).digest()
    payload = bytearray()
    payload += struct.pack(">Q", len(image))
    payload += b"sha256\0" + bytes(25)
    payload += struct.pack(">III", len(name), len(salt), len(digest))
    payload += struct.pack(">I", 0)
    payload += bytes(60)
    payload += name + salt + digest
    payload += bytes((-len(payload)) % 8)
    return struct.pack(">QQ", 2, len(payload)) + payload


def vbmeta_image(images: dict[str, bytes], rollback_index: int = 0) -> bytes:
    descriptors = bytearray()
    for name, image in images.items():
        salt = hashlib.sha256(("aosp-wear:" + name).encode()).digest()
        descriptors += avb_hash_descriptor(name, image, salt)
    aux_size = align_up(len(descriptors), 64)
    aux = bytes(descriptors) + bytes(aux_size - len(descriptors))
    header = bytearray(256)
    header[0:4] = b"AVB0"
    struct.pack_into(">I", header, 4, 1)
    struct.pack_into(">I", header, 8, 0)
    struct.pack_into(">Q", header, 12, 0)
    struct.pack_into(">Q", header, 20, aux_size)
    struct.pack_into(">I", header, 28, 0)
    struct.pack_into(">Q", header, 96, 0)
    struct.pack_into(">Q", header, 104, len(descriptors))
    struct.pack_into(">Q", header, 112, rollback_index)
    release = b"AOSP Wear OS Android 17 QPR1 dev"
    header[128:128 + len(release)] = release
    return bytes(header) + aux


def boot_control() -> bytes:
    raw = bytearray(32)
    raw[0:3] = b"_a\0"
    struct.pack_into("<I", raw, 4, BOOT_CTRL_MAGIC)
    raw[8] = 1
    raw[9] = 2
    raw[12] = 15 | (7 << 4)
    raw[14] = 14 | (7 << 4)
    struct.pack_into("<I", raw, 28, zlib.crc32(raw[:28]) & 0xFFFFFFFF)
    return bytes(raw)


def lp_geometry() -> bytes:
    raw = bytearray(52)
    struct.pack_into("<II", raw, 0, LP_GEOMETRY_MAGIC, 52)
    struct.pack_into("<III", raw, 40, LP_METADATA_MAX, LP_METADATA_SLOTS, 4096)
    digest = hashlib.sha256(raw).digest()
    raw[8:40] = digest
    return bytes(raw) + bytes(LP_GEOMETRY_SIZE - len(raw))


def fixed_name(name: str, size: int) -> bytes:
    b = name.encode("ascii")
    if len(b) >= size:
        raise ValueError(name)
    return b + bytes(size - len(b))


def lp_metadata(entries: list[dict], super_size: int) -> bytes:
    partitions = bytearray()
    extents = bytearray()
    groups = [
        ("aosp_wear_dynamic_partitions_a", 0, 48 * MIB),
        ("aosp_wear_dynamic_partitions_b", 0, 48 * MIB),
    ]
    for i, e in enumerate(entries):
        partitions += struct.pack(
            "<36sIIII",
            fixed_name(e["name"], 36),
            LP_PARTITION_ATTR_READONLY,
            i,
            1,
            e["group"],
        )
        extents += struct.pack("<QIQI", e["sectors"], 0, e["first_sector"], 0)
    group_blob = b"".join(struct.pack("<36sIQ", fixed_name(n, 36), flags, maximum) for n, flags, maximum in groups)
    first_logical_sector = min(e["first_sector"] for e in entries)
    block_devices = struct.pack(
        "<QIIQ36sI",
        first_logical_sector,
        ALIGN,
        0,
        super_size,
        fixed_name("super", 36),
        0,
    )

    tables = bytearray()
    descriptors = []
    for blob, entry_size, count in (
        (bytes(partitions), 52, len(entries)),
        (bytes(extents), 24, len(entries)),
        (group_blob, 48, len(groups)),
        (block_devices, 64, 1),
    ):
        descriptors.append((len(tables), count, entry_size))
        tables += blob

    header = bytearray(LP_HEADER_SIZE)
    struct.pack_into("<IHHI", header, 0, LP_HEADER_MAGIC, 10, 0, LP_HEADER_SIZE)
    struct.pack_into("<I", header, 44, len(tables))
    header[48:80] = hashlib.sha256(tables).digest()
    off = 80
    for desc in descriptors:
        struct.pack_into("<III", header, off, *desc)
        off += 12
    check = bytearray(header)
    check[12:44] = bytes(32)
    header[12:44] = hashlib.sha256(check).digest()
    blob = bytes(header) + bytes(tables)
    if len(blob) > LP_METADATA_MAX:
        raise RuntimeError("liblp metadata exceeds metadata_max_size")
    return blob + bytes(LP_METADATA_MAX - len(blob))


def make_super(output: Path, logical_images: dict[str, bytes]) -> None:
    entries = []
    cursor = ALIGN
    for name, size_mib, group in LOGICAL_LAYOUT:
        cursor = align_up(cursor, ALIGN)
        size = size_mib * MIB
        entries.append({
            "name": name,
            "offset": cursor,
            "first_sector": cursor // SECTOR,
            "sectors": size // SECTOR,
            "size": size,
            "group": group,
        })
        cursor += size
    if cursor > SUPER_SIZE:
        raise RuntimeError("logical layout exceeds super")

    meta = lp_metadata(entries, SUPER_SIZE)
    geometry = lp_geometry()
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("wb") as f:
        f.truncate(SUPER_SIZE)
        f.seek(LP_RESERVED_BYTES)
        f.write(geometry)
        f.seek(LP_RESERVED_BYTES + LP_GEOMETRY_SIZE)
        f.write(geometry)
        primary_base = LP_RESERVED_BYTES + LP_GEOMETRY_SIZE * 2
        backup_base = primary_base + LP_METADATA_MAX * LP_METADATA_SLOTS
        for slot in range(LP_METADATA_SLOTS):
            f.seek(primary_base + slot * LP_METADATA_MAX)
            f.write(meta)
            f.seek(backup_base + slot * LP_METADATA_MAX)
            f.write(meta)
        for e in entries:
            image = logical_images[e["name"]]
            if len(image) > e["size"]:
                raise RuntimeError(f"{e['name']} payload exceeds logical partition")
            f.seek(e["offset"])
            f.write(image)


def protective_mbr(total_lba: int) -> bytes:
    mbr = bytearray(512)
    entry = bytearray(16)
    entry[4] = 0xEE
    struct.pack_into("<I", entry, 8, 1)
    struct.pack_into("<I", entry, 12, min(total_lba - 1, 0xFFFFFFFF))
    mbr[446:462] = entry
    mbr[510:512] = b"\x55\xaa"
    return bytes(mbr)


def guid_le(g: uuid.UUID) -> bytes:
    return g.bytes_le


def partition_entry(name: str, first: int, last: int, disk_guid: uuid.UUID) -> bytes:
    e = bytearray(128)
    e[0:16] = guid_le(BASIC_DATA_GUID)
    e[16:32] = guid_le(uuid.uuid5(disk_guid, name))
    struct.pack_into("<Q", e, 32, first)
    struct.pack_into("<Q", e, 40, last)
    n = name.encode("utf-16le")[:72]
    e[56:56 + len(n)] = n
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

    backup_entries_lba = total_lba - GPT_ENTRIES_SECTORS - 1
    return hdr(1, total_lba - 1, 2), bytes(entries), hdr(total_lba - 1, 1, backup_entries_lba), backup_entries_lba


def write_sd_image(output: Path, size_mib: int, blobs: dict[str, bytes], version: str) -> None:
    layout = [
        ("misc", 4), ("metadata", 16),
        ("boot_a", 8), ("boot_b", 8),
        ("init_boot_a", 8), ("init_boot_b", 8),
        ("vendor_boot_a", 16), ("vendor_boot_b", 16),
        ("vbmeta_a", 1), ("vbmeta_b", 1),
        ("super", 128),
        ("swap", 32),
    ]
    total_bytes = size_mib * MIB
    total_lba = total_bytes // SECTOR
    cursor = ALIGN_LBA
    parts = []
    for name, mib in layout:
        cursor = align_up(cursor, ALIGN_LBA)
        blocks = mib * MIB // SECTOR
        parts.append((name, cursor, cursor + blocks - 1))
        cursor += blocks
    cursor = align_up(cursor, ALIGN_LBA)
    backup_entries_lba = total_lba - GPT_ENTRIES_SECTORS - 1
    if cursor + ALIGN_LBA >= backup_entries_lba:
        raise RuntimeError("disk image too small")
    parts.append(("userdata", cursor, backup_entries_lba - 1))

    disk_guid = uuid.uuid5(DISK_NAMESPACE, f"aosp-wear-android-{version}")
    primary, entries, backup, backup_entries = gpt_headers(total_lba, parts, disk_guid)
    by_name = {name: (first, last) for name, first, last in parts}
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("wb") as f:
        f.truncate(total_bytes)
        f.seek(0); f.write(protective_mbr(total_lba))
        f.seek(SECTOR); f.write(primary)
        f.seek(2 * SECTOR); f.write(entries)
        f.seek(backup_entries * SECTOR); f.write(entries)
        f.seek((total_lba - 1) * SECTOR); f.write(backup)

        def put(name: str, data: bytes, offset: int = 0):
            first, last = by_name[name]
            cap = (last - first + 1) * SECTOR
            if offset + len(data) > cap:
                raise RuntimeError(f"{name}: image exceeds partition")
            f.seek(first * SECTOR + offset)
            f.write(data)

        put("misc", boot_control(), 2048)
        for suffix in ("a", "b"):
            put(f"boot_{suffix}", blobs["boot.img"])
            put(f"init_boot_{suffix}", blobs["init_boot.img"])
            put(f"vendor_boot_{suffix}", blobs["vendor_boot.img"])
            put(f"vbmeta_{suffix}", blobs["vbmeta.img"])
        put("super", blobs["super.img"])


def verify_super(super_path: Path) -> None:
    data = super_path.read_bytes()
    geo = data[LP_RESERVED_BYTES:LP_RESERVED_BYTES + 52]
    if struct.unpack_from("<I", geo, 0)[0] != LP_GEOMETRY_MAGIC:
        raise RuntimeError("super geometry magic")
    check = bytearray(geo)
    expected = bytes(check[8:40])
    check[8:40] = bytes(32)
    if hashlib.sha256(check).digest() != expected:
        raise RuntimeError("super geometry checksum")
    off = LP_RESERVED_BYTES + LP_GEOMETRY_SIZE * 2
    hdr = bytearray(data[off:off + LP_HEADER_SIZE])
    if struct.unpack_from("<I", hdr, 0)[0] != LP_HEADER_MAGIC:
        raise RuntimeError("super metadata magic")
    expected_h = bytes(hdr[12:44]); hdr[12:44] = bytes(32)
    if hashlib.sha256(hdr).digest() != expected_h:
        raise RuntimeError("super header checksum")
    tables_size = struct.unpack_from("<I", hdr, 44)[0]
    tables = data[off + LP_HEADER_SIZE:off + LP_HEADER_SIZE + tables_size]
    if hashlib.sha256(tables).digest() != data[off + 48:off + 80]:
        raise RuntimeError("super tables checksum")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--output", default=str(ROOT / "out" / "target" / "product" / "zero"))
    ap.add_argument("--disk-size-mib", type=int, default=512)
    args = ap.parse_args()
    out = Path(args.output)
    out.mkdir(parents=True, exist_ok=True)
    version = (ROOT / "VERSION").read_text().strip()

    with tempfile.TemporaryDirectory(prefix="aosp-wear-fw-") as tmp_s:
        staging = Path(tmp_s)
        partition_payloads: dict[str, bytes] = {}
        for part in ("system", "system_ext", "vendor", "product", "odm"):
            tree = prepare_partition_tree(part, staging)
            image = cpio_newc(tree_files(tree))
            (out / f"{part}.img").write_bytes(image)
            partition_payloads[part] = image

        # Both slots start identical; OTA can replace the inactive extents later.
        logical_images = {}
        for name, _, _ in LOGICAL_LAYOUT:
            base = name.rsplit("_", 1)[0]
            logical_images[name] = partition_payloads[base]
            (out / f"{name}.img").write_bytes(partition_payloads[base])

        super_path = out / "super.img"
        make_super(super_path, logical_images)
        verify_super(super_path)

    init_ramdisk = cpio_newc({"init.rhai": (ROOT / "ramdisk" / "init.rhai").read_bytes()})
    boot = android_boot_v4(b"", version)
    init_boot = android_boot_v4(init_ramdisk, version)
    vendor_boot = vendor_boot_v4(version)
    vbmeta = vbmeta_image({"boot": boot, "init_boot": init_boot, "vendor_boot": vendor_boot}, rollback_index=1)

    blobs = {
        "boot.img": boot,
        "init_boot.img": init_boot,
        "vendor_boot.img": vendor_boot,
        "vbmeta.img": vbmeta,
        "super.img": super_path.read_bytes(),
    }
    for name, data in blobs.items():
        (out / name).write_bytes(data)

    sd = out / "aosp-wear-sd.img"
    write_sd_image(sd, args.disk_size_mib, blobs, version)

    manifest_files = [
        "boot.img", "init_boot.img", "vendor_boot.img", "vbmeta.img", "super.img",
        "system.img", "system_ext.img", "vendor.img", "product.img", "odm.img",
    ]
    manifest = {
        "product": "aosp_wear",
        "device": "zero",
        "version": version,
        "runtime": "rhai",
        "architecture": "xtensa-lx7",
        "dynamic_partitions": True,
        "avb_state": "orange-development",
        "images": {name: {
            "bytes": (out / name).stat().st_size,
            "sha256": hashlib.sha256((out / name).read_bytes()).hexdigest(),
        } for name in manifest_files},
    }
    (out / "build_manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")

    framework_size = len(bundle_framework(ROOT / "system"))
    init_size = (ROOT / "ramdisk" / "init.rhai").stat().st_size
    component_size = sum((ROOT / p).stat().st_size for p in (
        "vendor/etc/aosp_wear/vendor_runtime.rhai",
        "odm/etc/aosp_wear/odm_runtime.rhai",
        "system_ext/etc/aosp_wear/system_ext_runtime.rhai",
        "product/etc/aosp_wear/product_runtime.rhai",
    ))
    bundle_estimate = framework_size + init_size + component_size + 512
    print(f"AOSP Wear OS / Android 17 QPR1 ({version})")
    print(f"  output           : {out}")
    print(f"  boot.img         : {len(boot)} bytes")
    print(f"  init_boot.img    : {len(init_boot)} bytes")
    print(f"  vendor_boot.img  : {len(vendor_boot)} bytes")
    print(f"  vbmeta.img       : {len(vbmeta)} bytes (AVB algorithm NONE / ORANGE)")
    print(f"  super.img        : {SUPER_SIZE // MIB} MiB, Android liblp v10.0")
    print(f"  runtime bundle   : ~{bundle_estimate} bytes")
    print(f"  disk image       : {args.disk_size_mib} MiB raw GPT (sparse-capable)")
    if bundle_estimate > 32 * 1024:
        raise SystemExit("runtime bundle exceeds Bootloader 1.4 limit")


if __name__ == "__main__":
    main()
