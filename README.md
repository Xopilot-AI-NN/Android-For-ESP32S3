# AOSP Wear for ESP32-S3 — current release 17.2.1

**Platform identity:** Android 17 QPR1 / API 37  
**Project release:** 17.2.1  
**Target:** Waveshare ESP32-S3-Zero-N4R2 + ST7789V3 240×280

17.2.1 keeps the encoder-first Wear UI from 17.2.0, fixes Wi-Fi credential handling and adds a browser remote served directly by the ESP32-S3.

```bash
./flash pc  # Запуск с образом на ПК, без карты памяти
./flash sd  # Запуск с microSD
```

After Wi-Fi is online, open `http://<watch-ip>/` from a phone or laptop on the same LAN/hotspot. The URL is also shown by `wifi-status`.

## Logs and background work

The main loop sleeps through the RTOS between input polls. Static Material
scenes reuse the Rhai result while native clocks, PIN locking and timer alerts
continue ticking. Background screen checks normally run once per second,
and exact deadlines for an immediate dim step ten seconds before screen-off.
Viewer state is sent only after a viewer subscribes with SYNC/PING; identical
state is not repeatedly printed while the watch lies idle.

## Crown gestures and text input

Hold the crown button and rotate forward/back for right/left swipes. Releasing
after a gesture does not also click a row or trigger a long-press action.
The configured reverse direction applies to these gestures. Ordinary rotation
and button holds remain unchanged when no chord is used.

Device name and Wi-Fi password use one phone-style keyboard with QWERTY/ЙЦУКЕН
rows, RU/EN, Shift, digits, symbols, Space, Delete, Done and Cancel. Rotate to
focus, press to activate; the focus changes color without resizing keys.
Device names are limited to 32 UTF-8 bytes, passwords to 64. Voice input is a
future companion feature; a microphone button is not currently implemented.

Browser/Telegram architecture and current limitations are described in
[DELIVERY.md](Bootloader/DELIVERY.md); neither client is implemented yet.

## Ordinary Android ADB over Wi-Fi

The firmware has a limited native ADB TCP service. The system `adb` client
connects to it directly; the local `./adb` helper is optional.

```sh
adb connect 10.214.249.238:5555   # use the current address shown by the watch
adb logcat
adb logcat -d -v threadtime
adb logcat '*:W'
adb shell getprop ro.product.name
adb shell settings put global device_name 'AOSP Denis'
```

`logcat` streams the firmware's bounded 32-record RAM log ring with ACK pacing.
It supports `-d`, `-c`, `-t N`, `-T N`, `-s`, `-v brief|raw|time|threadtime`
and `Tag:I` filters. Logs are volatile; ROM/vendor panic output may only be
visible on serial. PID/TID are zero; timestamps before RTC sync use boot time.
This is an embedded service subset, not a Linux shell or Android userspace.

Product identity is centralized in `Bootloader/src/device_info.rs`, exposed
by the main module. `ro.product.name` and `ro.product.device` are `aosp`;
the editable friendly name is separate and saved on the card. The crown editor
is in **Настройки → Система → Имя устройства**. BLE advertisements refresh
when advertising restarts; a connected peer may cache its previous name.

## Wallpapers, launcher and AOD

Sixteen defaults from `Firmware/walp` are built into the loader. Select them in
**Настройки → Экран → Выбор обоев**. The watch face now displays them at 85%
before the user's overall brightness is applied.

To upload a custom image with ordinary ADB:

```sh
adb push picture_240x280.png /sdcard/Wallpapers/current.png
```

The restricted SYNC service accepts 240×280, 8-bit non-interlaced PNG files
up to 320 KiB, validates them and saves RGB565 to the reserved userdata area.
It does not expose a general filesystem. A successful transfer selects the
uploaded wallpaper; saving can fail if userdata is unavailable or occupied by
an incompatible filesystem. JPEG/other sizes must be converted on the PC first.
No card rewrite is needed. Defaults are restored by selecting one in Settings.

The launcher remembers grid/list mode across reopening and reboot. Focus uses
a darker fill without changing button dimensions. AOD is an optional switch
under Display: after the sleep deadline it shows only a dim clock on black,
updates by the minute and consumes the first input to wake. With AOD disabled,
the panel turns off as before. ST7789 is an LCD, so AOD keeps its backlight on
and is not equivalent to low-power OLED AOD. The PIN gate remains active.

## Optional local USB adapter

The project's local ZADB adapter additionally supports host-formatted logcat:

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

Current implementation status and limitations: [DELIVERY.md](Bootloader/DELIVERY.md).
