# Hardware recovery: ST7789V3 + microSD (source-only)

This patch targets Bootloader branch commit `b38161a8aaa28d23fa31dfaeb074a2645aabdc46`.
It **does not** flash ESP32-S3. The existing working device firmware remains unchanged.

## ST7789V3 240x280 physical wiring

| Panel pin | ESP32-S3 GPIO |
|---|---:|
| VCC | 3.3V |
| GND | GND |
| SCL/SCK | 1 |
| SDA/MOSI | 2 |
| RES/RST | 3 |
| DC | 4 |
| CS | 5 |
| BLK/BL | 6 |

Fixes the source's old GPIO1..6 ordering, uses 20 MHz SPI3 while validating
physical wires, and lights the backlight only after a full black frame.
`TFT_Y_OFFSET=20` and `TFT_MADCTL=0x00` are left unchanged. If the physical
panel still shows gray after flashing much later, capture boot logs and
check DC/CS/RST/SCK/MOSI signaling and panel power before changing offsets.

## microSD SPI2 physical wiring

| Module pin | ESP32-S3 GPIO |
|---|---:|
| 3.3V | 3.3V |
| GND | GND |
| CS | 10 |
| MOSI | 9 |
| CLK/SCK | 8 |
| MISO | 7 |

The pins were already correct. Init remains <=400 kHz with CS high for 80
clock edges; an idle-clock SPI failure now has an explicit diagnostic.
After init, switch to conservative 10 MHz instead of 20 MHz. This sacrifices
some throughput for signal margin on modules with longer leads.

**Important:** the project's default feature set enables `pc-block-boot`.
In that mode the microSD SPI2 code is not built or used. To test physical
microSD later, explicitly choose `--no-default-features --features
'rhai-runtime,display-st7789'`; simply running `flash.sh` would actually
flash the device, so do NOT run it as part of source recovery.

## Follow-up checks when hardware testing is desired

1. First run `python tools/check_board_wiring.py` and `./verify.sh` on a host
   with the ESP Rust toolchain (verify is build-only, not flash).
2. Check whether the panel boots without a gray flash and displays text.
3. Check SD initialization/partition reading with a known-good microSD image.
4. Brightness screen 28 is UI-only for now: physical PWM brightness control
   is **not implemented** in `Bootloader/src/display.rs` (BLK is simply on/off).
5. Screen 29 renders solid white at full enabled backlight. Avoid prolonged
   use on a panel that already shows retention or discoloration.
6. Firmware screen 30 still needs its Back handler in
   `Firmware/system/framework/src/40_input_manager.rhai`; only Home exits.
7. Bluetooth device pairing is not implemented by adding its visual screen.

Use `restore_esp32s3_sources.py` for a dry run, then `--apply` after
creating a local Git branch. Exact source snippets are checked before any
files are modified; the helper will stop if the branch has drifted.
