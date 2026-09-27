# Zephyr Android v0.3 bring-up

## Boot/UI flow

```text
ESP ROM -> Rust bootloader -> GPT/A-B/AVB -> init_boot -> super/liblp
      -> Rhai system_server -> Lock screen -> Watch face -> Launcher
      -> Messages / Quick Settings / Settings / Clock / Connectivity
```

The ST7789 UI is rendered by the native Rust RGB565 compositor. Rhai owns
framework policy/state; the renderer owns pixels, dirty regions and display I/O.
The desktop viewer receives the same semantic M3 scene and never becomes the
source of truth.

## ZPager (`swap` GPT partition)

The disk image contains a 32 MiB physical GPT partition named `swap`. ZPager
uses fixed 4096-byte pages with a `ZPG1` header, tag, payload length and CRC32.
Page zero is reserved for a future allocation journal.

This is **managed paging**, not Linux swap. ESP32-S3 does not have the Linux MMU
and kernel VM machinery needed to transparently evict arbitrary RAM pages.
Zephyr Android explicitly serializes Activity-local state/caches only when an
Activity actually leaves the foreground and restores them when that Activity is
revisited. Crown movement inside the same screen does **not** write to swap,
which avoids pointless flash/SD wear. The same code works against physical SD
and the PC-backed raw GPT disk.

## Wi-Fi / Bluetooth

`radio-services` is the native ESP32-S3 radio build:

```bash
./flash.sh st7789 pc radio
# or physical SD:
./flash.sh st7789 sd radio
```

It uses `esp-radio 0.17` + `esp-rtos 0.2`. Wi-Fi currently brings up STA mode and
performs a native AP scan when SystemUI enables Wi-Fi. BLE HCI is initialized
and retained for the Android Bluetooth service. GATT/phone-link and Wi-Fi
credential/IP management are the next framework/networking layer.

The default `off` build remains available while native radio bring-up is tested
on the real N4R2 board:

```bash
./flash.sh st7789 pc
```

## ADB / WADB

v0.3 contains an ADB service core with the standard 24-byte ADB packet header
codec (`CNXN`, `OPEN`, `OKAY`, `WRTE`, `CLSE`) plus a no-reset development
transport named **ZADB** over the existing PC block Unix bridge.

```bash
python zadb.py getprop
python zadb.py services
python zadb.py packages
python zadb.py dumpsys
```

ZADB does not reopen `/dev/ttyACM0`; `usb_block_server.py` remains the sole TTY
owner. Standard Android USB ADB bulk endpoints and TCP/5555 wireless debugging
are not falsely claimed complete yet; their service ABI is deliberately shared
with ZADB so those transports can be added without rewriting system services.
