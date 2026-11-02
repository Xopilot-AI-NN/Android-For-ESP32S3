# AOSP Wear for ESP32-S3 — release 17.1.2

Bug-fix and polish release on top of 17.1.1.

## Fixes

- Wireless ADB now reads the correct runtime bit (23), so enabling **Developer options → Wireless debug** starts TCP/5555 instead of depending on Bluetooth state.
- `dumpsys` Wi-Fi/Bluetooth/WADB flags use the correct state bits.
- Wi-Fi credentials moved from ZPager slot 16 to reserved metadata slot 64. Slot 16 belongs to the Date & time Activity and could overwrite the saved network in 17.1.1. Legacy credentials are migrated automatically.
- Last-known wall-clock time is kept in ZPager as an RTC holdover. No USB/PC time is injected; companion/manual time is still authoritative.
- Brightness, theme, DND, airplane mode, Wi-Fi/Bluetooth intent and wireless-debug setting persist across reboots.
- Launcher and settings now use vector Wear-style icons instead of letter/place-holder circles.
- Watch face has a compact status strip and retains the three complication slots.
- Removed dead Material renderer helpers that caused the `dead_code` warning.

## Hardware note

Metal heatsinks close to the ESP32-S3 board antenna can detune/shield the 2.4 GHz RF path. Keep metal away from the antenna keep-out area; cool only the package with the smallest practical isolated heatsink.
