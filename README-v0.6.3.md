# Zephyr Watch v0.6.3

Bootloader 1.9.3 / Firmware 0.6.3-dev.

Changes:
- warning-clean PC-block build: SD/fastboot-only code is feature-gated instead of compiled as dead code;
- removed unused helpers and the unused ADB SYNC constant;
- removed implicit `--monitor` from the cargo runner used by one-shot PC-block flashing;
- restored the Wi-Fi association sequence that was proven to work on the ESP32-S3 board in v0.4: SSID/password association with AP selection left to the Espressif station driver;
- scan results remain available to SystemUI/ZADB but are no longer used to pin BSSID/channel before association;
- existing Wi-Fi reconnect protection, ZPager, BLE and WADB remain enabled.

The compiler warnings in v0.6.2 were dead-code/configuration warnings and were not the cause of Wi-Fi association timeout.
