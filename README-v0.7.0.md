# AOSP Wear OS for ESP32-S3 v0.7.0

Bootloader 2.0.0 / Firmware 0.7.0-dev.

This project implements an Android/Wear-shaped firmware stack for the ESP32-S3-Zero-N4R2:
Android v4 boot images, GPT/A-B, AVB development verification, liblp dynamic partitions,
Rhai system_server/userspace, native RGB565 Wear Material 3 Expressive SystemUI, ZPager,
Wi-Fi/BLE services and wireless ADB.

## System surfaces

- lock screen and watch face
- expressive app launcher
- notification stream
- Wear-style Quick Settings
- Settings, Display, Connectivity, System and About
- Wi-Fi and Bluetooth detail pages
- Clock with a working 5-minute timer and stopwatch; embedded alarm toggle
- widgets/status cards
- media-control shell
- dynamic system accent themes
- crown navigation, long-press Home and desktop mirror
- host wall-clock synchronization over the existing USB bridge

## Android identity

```text
ro.product.model=AOSP Wear OS
ro.build.version.release=17
ro.build.version.sdk=37
ro.build.display.id=AOSP Wear OS Android 17 QPR1
ro.build.characteristics=watch
net.hostname=android-aosp-wear
```

## Build

```bash
./flash
```

For firmware-image validation only:

```bash
cd Firmware
./build.sh
./verify.sh
```

See `WEAR_OS_PARITY.md` for the boundary between native functionality and UI/API-compatible emulation.
