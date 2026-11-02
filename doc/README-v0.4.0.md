# AOSP-ESP32S3 v0.4.0 connectivity bring-up

This release turns the v0.3 radio toggles into usable development services.

## Wi-Fi

`esp-radio 0.17` owns the ESP32-S3 station. `smoltcp 0.12` now owns IPv4,
DHCP and the TCP transport used by wireless ADB.

From another terminal while `./flash` is running:

```bash
cd Bootloader
python zadb.py wifi-scan
python zadb.py wifi-connect "YOUR_SSID" "YOUR_PASSWORD"
sleep 3
python zadb.py wifi-status
```

A successful connection reports `state=Online ip=...`.
The profile is stored in the GPT `swap` backing store through ZPager and is
reused the next time Wi-Fi is enabled.

Wi-Fi association loss triggers automatic reconnect. Scan dwell is deliberately
longer than the old v0.3 bring-up scan and the driver waits for STA startup before
scanning, fixing the common false `0 AP(s)` result immediately after enabling RF.

## Wireless ADB

Turn ADB on in Quick Settings / Connect after Wi-Fi has an IP. The watch listens
on TCP port 5555.

Development adbd intentionally has no RSA authentication yet. Keep it on a
trusted LAN only.

```bash
adb connect WATCH_IP:5555
adb shell getprop
adb shell dumpsys
adb shell id
```

The transport implements the standard 24-byte ADB packet header and CNXN/OPEN/
OKAY/WRTE/CLSE path. It deliberately does not advertise `shell_v2`, so the host
uses the small legacy shell stream implemented on the watch.

## Bluetooth LE

Bluetooth now uses the real ESP32-S3 controller, not a UI-only flag.

```bash
python zadb.py bt-scan
sleep 7
python zadb.py bt-status
python zadb.py bt-connect AA:BB:CC:DD:EE:FF
python zadb.py bt-status
python zadb.py bt-disconnect
```

The watch also advertises `Zephyr Watch`. Discovery parses real LE advertising
reports and a connect request uses HCI LE Create Connection. The v0.4 transport
establishes the BLE link; GATT profiles / Android Phone Link notification sync
remain the next framework layer rather than being faked here.

## Stability fixes

- 64 KiB reclaimed internal SRAM remains reserved for RTOS/radio stacks.
- DHCP is gated until 802.11 association is actually up.
- Wi-Fi reconnects after link loss / association timeout.
- Bluetooth resumes advertising after scans and disconnects.
- PC viewer heartbeat changed from 1 s to 5 s.
- `@ZWUI|PONG` heartbeats are no longer spammed into the terminal log.
- duplicate post-boot `HELLO` was removed; viewer `SYNC` is authoritative.
- Wi-Fi SSIDs are sanitized before entering the line-oriented development protocol.
- serial server still exits cleanly instead of dumping a Python traceback after a board reset.

## One command

From the project root:

```bash
./flash
```

This builds Firmware, builds the full ST7789 + PC-block + radio boot runtime,
flashes it, starts the block server, and opens the viewer.
