# 17.1.6

- Default to the 80 MHz wearable CPU profile instead of maximum CPU clock.
- Enable esp-radio minimum modem power saving for lower Wi-Fi heat/current.
- Replace the Wi-Fi password character carousel with a visible four-page QWERTY/symbol keyboard covering printable ASCII.
- Add lowercase and password-symbol glyphs to the physical ST7789 font.
- Keep password bytes private; viewer protocol packs keyboard page + selected key into the existing editor byte.

# 17.1.4

- Fixed the Wi-Fi-on freeze in USB-PC-block builds: host writes now tolerate temporary ESP32-S3 USB-Serial-JTAG backpressure instead of terminating with `SerialTimeoutException`.
- Added complete-write framing for line and binary block replies.
- Enabling Wi-Fi no longer performs a synchronous active scan; scanning is explicit on the Available networks page.
- Preserved the software RTC/SNTP behavior from 17.1.3.

# 17.1.3

- Fixed Wireless ADB runtime bit: TCP/5555 now follows Developer options instead of Bluetooth.
- Fixed `dumpsys` Wi-Fi/Bluetooth/WADB bit reporting.
- Moved Wi-Fi credentials to reserved ZPager metadata slots and added migration from the legacy colliding slot.
- Added persistent Wear settings and last-known wall-clock holdover.
- Replaced launcher/settings placeholder letters with compact vector Wear-style icons.
- Added watch-face status indicators and removed dead renderer helpers.

# 1.9.3

- Clean PC-block build warnings by feature-gating SD/fastboot-only code and removing unused helpers.
- Remove implicit espflash monitor runner option in PC-block mode.
- Restore the proven v0.4 ESP32-S3 Wi-Fi association path: SSID/password only, with AP selection left to the driver.
- Keep scan results for diagnostics without pinning hotspot BSSID/channel.

# 1.9.2

- Wi-Fi association reliability: directed SSID scan before connect.
- Pin connection attempts to strongest matching AP BSSID/channel.
- Re-submitting the currently connected profile no longer tears down the link.
- Cleaner failed-connect retry scheduling.

# 1.9.0

- Reliable WADB shell streams and interactive `adb shell`.
- Correct ADB CLOSE semantics and atomic packet queueing.
- Firmware 0.6 identity/properties and extended userdebug shell.

# 1.8.1

- Reset the Wi-Fi STA state machine after association timeouts.
- Explicit connections now use disconnect/stop/config/start/connect sequencing.
- Increased association timeout to 30 seconds and added retry backoff.
- Prevents repeated connect calls from leaving esp-radio stuck in Associating.

# Changelog

## 1.8.0

- Zephyr Watch Android product identity for ADB (`zephyr_watch` / `zephyr`).
- Fixed ADB WRTE/OKAY/CLSE ordering so one-shot `adb shell` commands close correctly.
- Wear OS 6 / Pixel Watch inspired Material 3 Expressive renderer for ST7789 and desktop viewer.
- WADB status now reports `listener` and `client` separately.
- Graceful Ctrl+C in desktop viewer.
- Firmware 0.6.2-dev product/build properties.

# 1.6.2 — radio SRAM / stability hotfix

- Added a 64 KiB `#[ram(reclaimed)]` internal heap region for esp-rtos/radio stacks.
- Registered the reclaimed region after PSRAM so ordinary Rhai/SystemUI allocations spill to PSRAM first while `InternalMemory` can still use reclaimed SRAM.
- BLE HCI initialization is now lazy and happens only on the first Bluetooth enable.
- Added internal-heap guards and diagnostics before Wi-Fi/BLE start.
- Hardened the PC block server against serial write timeouts/disconnects.
- Cleaned the known conditional-import/style warnings in the default one-shot build.

## 1.7.0
- Real Wi-Fi station profiles, active scan tuning, association/reconnect and smoltcp DHCP.
- Development wireless adbd TCP transport on port 5555.
- Real BLE active discovery, Zephyr Watch advertising and HCI LE connect/disconnect.
- ZADB Wi-Fi/Bluetooth control commands.
- Reduced viewer heartbeat/log spam and duplicate post-boot HELLO.

## 1.9.0 / Firmware 0.6.2-dev
- Fixed legacy ADB shell stream lifecycle: CLOSE is no longer echoed back.
- ADB packets are now queued atomically to avoid partial 24-byte framing corruption.
- Added one-shot shell close watchdog for host compatibility.
- Added interactive `adb shell` prompt with line commands and `exit`.
- Disabled Nagle for WADB and enabled TCP keepalive/timeout.
- Expanded userdebug shell (`getprop`, `dumpsys`, `wm`, package/service queries).
