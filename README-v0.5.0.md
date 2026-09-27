# AOSP-ESP32S3 v0.5.0

Zephyr Watch development stack for ESP32-S3-Zero-N4R2.

## Highlights

- Bootloader 1.8.0 / Firmware 0.5.0-dev.
- Device identity: `Zephyr Watch`, codename `zephyr`, manufacturer `Xopilot`.
- Wear OS 6 / Pixel Watch inspired Material 3 Expressive SystemUI adapted to the 240x280 ST7789 panel.
- Real Wi-Fi STA + scan + association + DHCP over esp-radio/smoltcp.
- BLE HCI advertising, scan and connection bring-up.
- Wireless ADB on TCP/5555.
- Fixed one-shot ADB shell stream close handshake (`adb shell getprop`, `id`, `uname -a`, etc.).
- Clear WADB status fields: `listener=` and `client=` instead of the ambiguous `tcp5555=`.
- Graceful desktop viewer Ctrl+C without Python traceback.
- A/B + GPT + AVB development verification + liblp super partitions + ZPager backing store.

## Flash

From the project root:

```bash
./flash
```

No display/storage/radio flags are required for the normal development setup.

## Wi-Fi + WADB

```bash
cd Bootloader
python zadb.py wifi-scan
python zadb.py wifi-connect "SSID" "PASSWORD"
python zadb.py wifi-status
```

Enable WADB on the watch, then:

```bash
adb connect WATCH_IP:5555
adb devices -l
adb shell getprop ro.product.model
adb shell getprop
adb shell id
adb shell uname -a
```

The development adbd still has no RSA authentication and should only be used on a trusted network.

## UI note

Pixel Watch hardware is round while this development panel is 240x280 rectangular. The v0.5 renderer therefore follows the current Wear OS 6 visual language (AMOLED black surfaces, dynamic system color, large centered clock, pill-shaped glanceable actions, expressive selected states) rather than pretending the physical geometry is identical.
