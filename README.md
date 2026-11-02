# AOSP Wear for ESP32-S3 — current release 17.2.1

**Platform identity:** Android 17 QPR1 / API 37  
**Project release:** 17.2.1  
**Target:** Waveshare ESP32-S3-Zero-N4R2 + ST7789V3 240×280

17.2.1 provides an encoder-first Wear interface with on-watch weather, native
utilities and ordinary wireless ADB. Browser remote control has been removed.

```bash
./flash pc  # Запуск с образом на ПК, без карты памяти
./flash sd  # Запуск с microSD
```

The watch does not run an HTTP control server. ADB remains available on port
5555 when enabled; weather uses outgoing public API requests.

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

## Watch utilities and device diagnostics

The launcher has 30 px Material icons and 15 entries. Native utilities add a
calculator, a built-in wallpaper browser, Wi-Fi/ADB transfer instructions,
device diagnostics, memory/swap controls, weather, world clock, breathing and a
flashlight shortcut. The file browser covers the
built-in PNG assets; it is not a general FAT/ext4 filesystem browser or Android
Quick Share implementation. The three-column grid scrolls vertically by rows instead of horizontal pages.

Open **Настройки → Система → Диагностика** for live heap/internal-memory,
uptime, Wi-Fi/BLE state, last render timings and cached storage capacities.
Temperature/current/battery measurements are unavailable until their drivers
or measurement hardware are added. Useful ordinary ADB commands:

```sh
adb shell dumpsys meminfo
adb shell dumpsys hardware
adb pull /sdcard/Wallpapers/1.png ./watch-wallpaper.png
```

System PNGs 1.png through 16.png are read-only and stream in ACK-paced 512-byte
chunks after an on-demand disk read. Display settings now persist wallpaper brightness and the analog second
hand switch. Hidden timer/stopwatch ticks no longer invalidate static pages.
For scope, limitations, power-measurement guidance and loader-only update
commands, see [delivery notes](Bootloader/DELIVERY.md).

## System graphics update

The loader now requires the resource package from the matching system image for
normal app icons and default wallpapers. Rebuild Firmware along with Bootloader;
for PC mode, `cd Bootloader && ./flash pc` performs both builds. For SD mode,
update the system image on the card as well as flashing the loader with
`./flash sd`; that command does not write a card inserted in a PC card reader.
A loader-only update against an older system image leaves normal graphics
unavailable and logs `Resources: system image needs UI resource update`.
Boot/recovery/fastboot lettering and drawing remain available without that pack.

## Wallpapers, launcher and AOD

Sixteen defaults from `Firmware/walp` and both icon atlases are packaged in
`system/usr/share/aosp-wear/ui-resources.bin` inside `super.img`. They are not
linked into the ESP32 executable. Select wallpapers in
**Настройки → Экран → Выбор обоев**. Wallpaper brightness defaults to 85%
and is configurable independently of the overall display brightness.

To upload a custom image with ordinary ADB:

```sh
adb push picture_240x280.png /sdcard/Wallpapers/current.png
```

The restricted SYNC service accepts 240×280, 8-bit non-interlaced PNG files
up to 320 KiB, validates them and saves RGB565 to the reserved userdata area.
It does not expose a general filesystem. A successful transfer selects the
uploaded wallpaper; saving can fail if userdata is unavailable or occupied by
an incompatible filesystem. JPEG/other sizes must be converted on the PC first.
Custom wallpaper uploads require no card rewrite. Defaults are restored by selecting one in Settings.

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

## Expand userdata on a larger microSD

Keep the 512 MiB installation image and expand only its last userdata partition
on the actual card. System/swap sizes, partition starts and existing payloads
are preserved; this does not create a filesystem or add more RAM.
The Linux helper validates the watch GPT layout and refuses mounted/busy cards.
Check the card path with lsblk; /dev/sdX below is a placeholder.

```sh
sudo python3 Firmware/tools/grow_userdata.py --device /dev/sdX
sudo python3 Firmware/tools/grow_userdata.py --device /dev/sdX --apply --backup-dir Bootloader/out/gpt-backups
```

The first command is read-only. The second backs up affected GPT/MBR sectors,
then grows userdata and relocates backup GPT. This is a metadata backup, not a
full data backup. Rewriting the full installation image afterwards resets the
partition layout. The helper does not format or resize a filesystem.

## Adjustable ZPager backing

Open **Настройки → Система → Память и swap** for total backing sizes
32/64/128/256/512 MiB. Selection is persisted, applied after reboot and limited
by the available supported raw userdata range. The base 32 MiB GPT swap and
existing persistent slots keep their addresses; additional pages use a reserved
userdata region starting at 8 MiB. This is not Linux/MMU swap or extra RAM.
Existing Activity consumers still use their original low page numbers; larger
backing does not automatically page out the Rust/Rhai heap or improve speed.
The 512 MiB image supports at most 256 MiB backing; 512 MiB backing requires
userdata of at least 504 MiB (grow it on the card first). No FAT/ext4 swapfile
allocation is implemented. Shrinking the active limit retains the previous raw
reservation; it does not reclaim userdata for a future filesystem.

```sh
adb shell dumpsys swap
```

## Startup, larger text and weather

A short startup screen remains visible for at least two seconds while local
graphics/Bluetooth settle, with an eight-second cap. Network association may
continue in the background. UI Roboto is now 14/18 px with a separate 36 px
antialiased atlas; app icons use colored circular backgrounds.

Open **Погода → Город** to search with the on-watch keyboard, **Рядом** for
approximate IP-based location, or **Обнов.** for the last selected location.
The watch itself uses Wi-Fi/DNS/HTTP to reach IP-API and Open-Meteo; no PC helper
is required. These public API calls currently use HTTP, not HTTPS. Location is
persisted, forecasts are held in RAM, failures are shown explicitly. There is
no idle automatic weather polling. See [delivery notes](Bootloader/DELIVERY.md)
for provider links, limits and update instructions.
