# AOSP Wear for ESP32-S3 — current release 17.2.1

**Platform identity:** Android 17 QPR1 / API 37  
**Project release:** 17.2.1  
**Target:** Waveshare ESP32-S3-Zero-N4R2 + ST7789V3 240×280

17.2.1 keeps the encoder-first Wear UI from 17.2.0, fixes Wi-Fi credential handling and adds a browser remote served directly by the ESP32-S3.

```bash
./flash pc - тестирование прошивки через пк без участия карты памяти
./flash sd - тестрирование на реальных устловиях 
```

After Wi-Fi is online, open `http://<watch-ip>/` from a phone or laptop on the same LAN/hotspot. The URL is also shown by `wifi-status`.

## Logs and background work

The main loop sleeps through the RTOS between input polls. Static Material
scenes reuse the Rhai result while native clocks, PIN locking and timer alerts
continue ticking. Background screen checks normally run once per second,
with faster checks during the short brightness fade before screen-off.
Viewer state is sent only after a viewer subscribes with SYNC/PING; identical
state is not repeatedly printed while the watch lies idle.

The project's local ZADB adapter supports Android-style logcat output:

```sh
./adb logcat                       # live logs; PC bridge if running, else USB
./adb logcat '*:W'                 # warnings and errors
./adb logcat -s radio:D            # one tag
./adb logcat -v brief
./adb logcat --serial /dev/ttyACM0  # direct USB when booting from SD
```

With the PC block server running, history is available without reopening its TTY:

```sh
./adb logcat -d       # dump the host's last 128 records
./adb logcat -t 30    # dump the last 30
./adb logcat -c       # clear host history
```

Stop an existing serial monitor before using direct USB. Direct mode does not
reset the watch and provides live logs; it has no historical device buffer.
Timestamps are host receive times. ESP-IDF log levels/tags and uptime are
preserved in records; older plain-text messages get inferred levels and tags.
PID/TID are zero because this firmware has no Android process IDs.
`./adb` is this repository's adapter, not the system Android ADB/adbd protocol.
It needs the same Python/pyserial environment as the existing flash tools.

Loader-only SD update (the card image need not be rewritten):

```sh
AOSP_WEAR_SKIP_FIRMWARE_BUILD=1 ./flash sd
```

See [`README-v17.2.1.md`](README-v17.2.1.md) and [`WEAR_OS_PARITY.md`](WEAR_OS_PARITY.md).
