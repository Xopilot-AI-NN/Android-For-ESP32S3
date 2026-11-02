# Zephyr Watch v0.6.1

Wi-Fi reliability hotfix on top of v0.6.0.

- Bootloader 1.9.1 / Firmware 0.6.1-dev.
- Do not tear down a healthy Wi-Fi link when the same profile is submitted again.
- Before association, perform a directed scan for the requested SSID.
- Lock the station configuration to the strongest AP BSSID + channel when found.
- Fall back to SSID-only association if the directed scan fails.
- Failed connect requests schedule a clean retry instead of leaving stale association state.
- Keeps the v0.6 ADB stream fixes unchanged.

Run `./flash` from the repository root.
