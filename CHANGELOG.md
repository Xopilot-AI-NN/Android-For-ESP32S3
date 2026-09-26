# Changelog

## 1.1.0

- No-reset USB Desktop attach: DTR/RTS are set inactive before opening CDC.
- Linux `HUPCL` is disabled when the tty driver supports it.
- Desktop viewer serial I/O moved to a background worker.
- Automatic reconnect after unplug/replug or `/dev/ttyACM*` re-enumeration.
- Protocol bumped to Remote Surface v2.
- Added liveness `PING/PONG` messages.
- Fixed board-side loss of multiple desktop commands arriving in one USB packet by adding a fixed event queue.
- Fastboot `SYNC` now re-sends HELLO + current Fastboot UI.

## 1.0.0

- ST7789V3 default display backend.
- SSD1306 optional backend.
- USB Desktop Surface v1.
- SD/FAT A/B boot with SHA-256 manifests and Rhai runtime.
- Recovery and Fastboot+ modes.

## 1.2.0 — Android storage/boot transition

- Replaced FAT `SLOTA/SLOTB` as the primary boot path with a real GPT microSD layout.
- Added AOSP-style physical partitions: `misc`, `metadata`, `boot_a/b`, `init_boot_a/b`,
  `vendor_boot_a/b`, `vbmeta_a/b`, `super`, `userdata`.
- Added GPT header + partition-entry CRC32 validation in the Rust bootloader.
- Added AOSP `bootloader_control` compatible 32-byte A/B metadata in `misc` at offset 2048.
- Added priority, `tries_remaining`, `successful_boot`, corruption-aware unbootable marking and automatic A/B fallback.
- Added Android boot image header v3/v4 parser (`ANDROID!`).
- Added Android vendor boot v4 validation (`VNDRBOOT`, 4096-byte page, 2128-byte v4 header).
- Early Rhai userspace now comes from an uncompressed `newc` CPIO ramdisk in `init_boot_*` as `/init.rhai`.
- Added AVB `AVB0` parser for SHA-256 hash descriptors and verifies `boot`, `init_boot`, and `vendor_boot`.
- Development vbmeta uses real AVB structures with `algorithm NONE`; boot UI reports **AVB ORANGE**.
- Added `tools/mk_android_sd.py` to generate a flashable sparse raw GPT SD image.
- Added `tools/inspect_android_sd.py` to validate GPT, A/B metadata, Android image magics and AVB digests.
- SD starts at 400 kHz for card initialization and is re-clocked to 20 MHz for normal boot I/O.
- Added `ANDROID_BOOT.md` as the explicit boot ABI and security/feature boundary.
- Old FAT slot implementation moved to `legacy/fat-slot-v1/` and is no longer in the boot path.
