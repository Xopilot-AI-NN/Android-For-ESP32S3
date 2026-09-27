# AOSP Wear for ESP32-S3 — release 17.1.3

17.1.3 focuses on making connectivity and timekeeping usable from the watch itself.

## Wi-Fi from the watch

The crown-only UI now supports the full station flow:

`Settings → Connectivity → Wi-Fi → Networks → SSID → Password → Connect`

- entering **Networks** performs a native ESP32-S3 Wi-Fi scan;
- rotate the crown to select an access point;
- press to open the password editor;
- rotate to choose a character and press to append it;
- `DEL`, `CONNECT`, and `BACK` are part of the same character wheel;
- holding the crown on the password screen is a shortcut for **Connect**;
- the connection screen follows association → DHCP → online state;
- successful credentials are persisted and are used for later auto-connect.

`Bootloader/zadb.py wifi-connect` remains available as a development path but is no longer required for normal setup.

## Software RTC and network time

Wall-clock time now lives in the dedicated `Bootloader/src/rtc.rs` module.

- The RTC ticks from the ESP monotonic counter and does not depend on USB or the desktop viewer.
- Before the first authoritative sync the watch still displays a ticking software clock instead of `--:--` and marks it as **SYNCING TIME**.
- Last-known valid local time is persisted as a reboot holdover.
- Automatic time uses SNTP over Wi-Fi as soon as DHCP is online.
- The SNTP client retries alternate Google Public NTP IPv4 endpoints and re-syncs every six hours.
- Manual time mode is not overwritten by SNTP. Re-enabling **Automatic time** requests an immediate fresh SNTP update.
- The automatic-time flag and timezone setting are persisted independently from Activity pages.

The ESP32-S3 board has no battery-backed calendar RTC. A full power loss therefore cannot account for elapsed power-off time; the saved holdover resumes immediately and Wi-Fi SNTP corrects it once Internet access returns.

## Other fixes

- Wireless ADB remains tied to runtime bit 23 and listens on TCP/5555 only when explicitly enabled.
- Wi-Fi credentials, RTC holdover, persistent UI settings, and RTC configuration use reserved ZPager metadata slots 64–67.
- Wi-Fi pages and the desktop viewer mirror the new scan/password/connection states.
- The watch face, lock screen, Quick Settings, and notification header show software-RTC digits even before the first sync.
