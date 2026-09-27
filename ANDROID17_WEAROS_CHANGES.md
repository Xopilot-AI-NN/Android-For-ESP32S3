# Android 17 QPR1 / AOSP Wear OS update

This patch keeps the ESP32-S3/Rhai architecture but changes the externally visible
Android identity and SystemUI presentation.

## Networking

- DHCP client sends option 12 hostname: `android-aosp-wear`.
- Wireless ADB accepts the checksum-less packets used by ADB protocol 0x01000001,
  fixing modern `adb shell` / `adb shell getprop` packet checksum mismatches.
- ADB identity is now `product:aosp_wear model:AOSP Wear OS device:aosp_wear`.

## Android identity

- Android release: `17`
- SDK/API: `37`
- Display ID: `AOSP Wear OS Android 17 QPR1`
- Build type: `userdebug`
- Hostname: `android-aosp-wear`
- BLE device name: `AOSP Wear OS`

This is an AOSP-style compatibility identity for this ESP32-S3 runtime; it does
not turn the board into Google's certified Wear OS distribution.

## UI

The ST7789 renderer and desktop viewer now use a Wear OS-style Material 3 layout:
large watch-face clock, compact status chrome, complications, round quick-setting
buttons, curved/expanded list selection, Android System notifications, and a
System/About screen identifying Android 17 QPR1 / API 37.

## Validation

`Firmware/verify.sh` passes for the regenerated boot images, GPT, AVB descriptors,
liblp super image and Rhai runtime bundle. Rust/ESP compilation still needs to be
run on a machine with the project's Rust toolchain installed.
