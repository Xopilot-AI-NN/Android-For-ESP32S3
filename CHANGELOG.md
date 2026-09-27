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
