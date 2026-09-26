# Zephyr Watch Android-style Boot ABI v1

This document is the contract between the ESP32-S3 Rust bootloader and the
Rhai/AOSP-like firmware stored on microSD.

## Trust / boot chain

```text
ESP32-S3 ROM
  -> Zephyr Rust bootloader (internal SPI flash)
  -> microSD GPT
  -> misc / bootloader_control
  -> vbmeta_<slot>
  -> boot_<slot> + init_boot_<slot> + vendor_boot_<slot>
  -> /init.rhai from init_boot CPIO
  -> Rhai early userspace
```

The operating system is installed on microSD. Internal flash is intentionally
kept for the immutable/rarely-updated boot chain and future trusted state.

## Physical GPT partitions

The bootloader currently requires these physical names:

- `misc`
- `metadata`
- `boot_a`, `boot_b`
- `init_boot_a`, `init_boot_b`
- `vendor_boot_a`, `vendor_boot_b`
- `vbmeta_a`, `vbmeta_b`
- `super`
- `userdata`

`super` is intentionally not parsed by the bootloader. It is reserved for the
userspace dynamic-partition layer (`system`, `system_ext`, `vendor`, `product`,
`odm`) so bootloader-required images stay physical.

## GPT rules

- Logical block size: 512 bytes.
- Protective MBR at LBA 0.
- Primary GPT header at LBA 1.
- GPT header CRC32 is mandatory.
- Partition-entry array CRC32 is mandatory.
- Partition lookup is by GPT UTF-16LE name (ASCII subset in bootloader v1).
- A backup GPT is emitted by the host image builder. Bootloader v1 validates
  the primary GPT; backup-GPT recovery is a later hardening item.

## A/B state (`misc`)

AOSP-compatible `bootloader_control` is stored at byte offset 2048 in `misc`.
The 32-byte structure uses:

- magic `0x42414342` (`BCAB` in little endian storage),
- version 1,
- 2 slots,
- priority,
- `tries_remaining`,
- `successful_boot`,
- `verity_corrupted`,
- CRC32 over the first 28 bytes.

Default state:

- slot A: priority 15, 7 tries,
- slot B: priority 14, 7 tries.

A transient card/GPT I/O error consumes the current attempt but does **not**
permanently zero slot priority. Proven AVB/image corruption may mark a slot
unbootable immediately and trigger the other slot.

## Android boot images

### `boot_<slot>`

- Android magic: `ANDROID!`
- Header v4 in generated images.
- Page size contract: 4096 bytes.
- Linux kernel payload is empty by design: hardware/native boot runtime is
  Rust in internal flash.

### `init_boot_<slot>`

- Android boot header v4 (`ANDROID!`).
- Generic ramdisk is an uncompressed `newc` CPIO.
- Required entrypoint: `/init.rhai` (also accepts `init.rhai`/`./init.rhai`).
- Max early-userspace script size: 24 KiB in bootloader v1.
- Command line may expose `androidboot.zephyr.version=<version>`.

### `vendor_boot_<slot>`

- Magic: `VNDRBOOT`.
- Header v4.
- Page size: 4096.
- Header size: 2128 bytes.
- Vendor ramdisk/DTB are reserved for later board-HAL growth.

## AVB

`vbmeta_<slot>` uses the actual `AVB0` header and AVB hash descriptors.
Bootloader v1 verifies SHA-256 descriptors for these logical descriptor names:

- `boot`
- `init_boot`
- `vendor_boot`

The digest is calculated using AVB semantics (`SHA256(salt || image)`), using
`image_size` from each descriptor.

### Development security state

Current generated vbmeta uses `AVB_ALGORITHM_TYPE_NONE`. Hashes are checked,
but vbmeta itself is not authenticated by a trusted public key. Therefore this
is **ORANGE/development state**, not secure/green Verified Boot.

`rollback_index` is parsed and surfaced but is **not yet enforced** against a
trusted monotonic counter. Production green state requires signed vbmeta plus a
trusted key and rollback state in ESP32-S3 protected internal storage/eFuse.

## SD bus speed

Initialization is performed at 400 kHz after the required idle clocks. After
`embedded-sdmmc` has initialized the card, the bootloader reconfigures SPI2 to
20 MHz for GPT/AVB/image reads.

## Rhai success handshake

For the v1 bootstrap, `/init.rhai` returning integer `0` marks the selected A/B
slot successful. This is deliberately temporary. Once `system_server` exists,
`markBootSuccessful()` must be moved to a later explicit boot-complete signal
rather than the return of early init.

## Not implemented yet

These are intentionally outside Boot ABI v1 rather than silently emulated:

- RSA-authenticated AVB green state / trusted-key provisioning.
- Persistent rollback-index enforcement.
- Android LP metadata and dynamic logical partitions inside `super`.
- Binary Android fastboot protocol and `fastboot flash` to microSD GPT.
- Backup-GPT automatic recovery.
- Recovery image/userspace and OTA/update_engine implementation.

The existing Fastboot+ text/USB recovery console remains available for
bring-up and diagnostics.
