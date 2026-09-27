# USB-PC block boot (development mode)

This mode exists so the ESP32-S3-Zero can boot the Android-like GPT image before
the microSD socket is soldered.

## Why this is not USB-host mode

The ESP32-S3 itself supports USB OTG host mode, but the current
ESP32-S3-Zero board USB-C connector is wired as the normal device/power-sink
port. Its CC pins use fixed sink resistors and the board has no host VBUS power
switch/role controller. A software driver cannot safely turn that connector
into a standards-compliant powered host port.

`pc-block-boot` therefore keeps the board in USB device mode. The PC powers the
board and serves a raw 512-byte block device over USB-Serial-JTAG. The raw file
is exactly the same GPT image that can later be written to a microSD card.

## Build / flash

ST7789:

```bash
./flash.sh st7789 pc
```

SSD1306:

```bash
./flash.sh ssd1306 pc
```

The script automatically creates `out/aosp-wear-sd.img` when it does not
exist.

## Start the virtual disk

Do not run `espflash --monitor` at the same time because both programs would
compete for the same USB serial device.

```bash
python usb_block_server.py --image out/aosp-wear-sd.img
```

The server auto-detects the first `/dev/ttyACM*` or `/dev/ttyUSB*`. To select a
specific port:

```bash
python usb_block_server.py \
  --port /dev/ttyACM0 \
  --image out/aosp-wear-sd.img
```

If the bootloader reached `PC DISK ERROR` before the server connected, press the
board RESET/EN button after the server is running.

## What is tested

There is no special fake filesystem in this mode. The normal boot pipeline is
used:

```text
USB-PC raw block device
        |
        v
protective MBR + GPT
        |
        v
misc / A-B bootloader_control
        |
        v
vbmeta_a/b + AVB SHA-256
        |
        v
boot_a/b + init_boot_a/b + vendor_boot_a/b
        |
        v
/init.rhai from init_boot ramdisk
        |
        v
Rhai runtime
```

Writes are enabled because the A/B metadata in `misc` must be updated. The
server writes those sectors back into the image, so after reboot the state is
preserved exactly like it will be on microSD.

## Protocol

The transport is deliberately tiny and development-only:

```text
@ZBLK|SIZE
@ZBLK|READ|<lba>|<count>
@ZBLK|WRITE|<lba>|<count>|<bytes>|<crc32>
```

Data transfers have CRC32 framing. GPT, AVB and image verification still happen
inside the Rust bootloader.

## Limitations

- PC must stay attached during any storage access.
- Desktop viewer and Fastboot+ are unavailable while `pc-block-boot` owns the
  USB transport.
- This is not the final watch storage path. The release hardware still uses
  microSD.
- It is not USB Mass Storage host mode and does not make the existing USB-C
  connector capable of safely powering a USB flash drive.

## Live viewer without resetting the board (v1.3.1+)

In `pc` mode only `usb_block_server.py` opens the ESP32 serial device. It also
creates a local Unix socket, normally:

```text
$XDG_RUNTIME_DIR/aosp-wear-viewer.sock
```

Start the viewer in another terminal:

```bash
python desktop_viewer.py
```

The viewer detects the socket and attaches to it instead of opening
`/dev/ttyACM0`. Closing/reopening the GUI therefore does not toggle serial
control lines, steal bytes from `@ZBLK`, or reset/re-enumerate the controller.

Use `python desktop_viewer.py --direct-serial` only for the normal SD-card path
when no PC-block server owns the TTY.
