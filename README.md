# Zephyr Android Firmware 0.4

AOSP-like Rhai userspace for **Zephyr Watch / ESP32-S3-Zero-N4R2**.

## Current system

- Android-style GPT and A/B boot images.
- AVB hash verification (development ORANGE / unsigned vbmeta).
- Android liblp `super` with `system`, `system_ext`, `vendor`, `product`, `odm` A/B logical partitions.
- Rhai `init`, ServiceManager, `system_server`, PackageManager, ActivityManager, Power/Input services.
- Connectivity/Wi-Fi/Bluetooth/ADB/ZPager service model.
- Material 3 RGB SystemUI rendered natively in Rust.
- Boot starts at a **lock screen**, then watch face, app launcher, messages,
  quick settings, settings, clock, connectivity and about screens.
- 32 MiB managed ZPager backing partition in the raw GPT image.

The logical partition payloads are compact read-only `newc` CPIO during bring-up.
Moving read-only system partitions to EROFS remains a later storage milestone.

## Build

```bash
./build.sh
./verify.sh
```

Output is under `out/target/product/zero/`, including:

```text
boot.img
init_boot.img
vendor_boot.img
vbmeta.img
super.img
zephyr-watch-sd.img
```

For the no-solder PC-block flow:

```bash
cd ../Bootloader
./flash.sh st7789 pc
python desktop_viewer.py     # second terminal
```

For experimental native Wi-Fi + BLE HCI bring-up:

```bash
./flash.sh st7789 pc radio
```
