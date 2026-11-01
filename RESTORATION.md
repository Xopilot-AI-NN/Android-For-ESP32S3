# Photo-based watch restoration

The supplied [photographs](../images/) and subsequent Wear UI reference images
guide the 240 × 280 ST7789V3 layout. This is an ESP32-S3 Rust/Rhai watch runtime;
it does not run Google's Wear OS or its proprietary services. Google's
[Wear color guidance](https://developer.android.com/design/ui/wear/guides/styles/color)
documents Material 3 Expressive and dynamic color. A verifiable Pixel Watch 5 /
Wear OS 7 reference was not available, so this work does not claim an exact
reproduction of those products.

## Restored behavior

The default system language is **Russian (`ru-RU`)**. System labels use
Russian translations and the Roboto atlases include all Russian Cyrillic
letters, including Ё/ё. Wi-Fi names and entered passwords bypass translation;
protocol keys, product names, Wi-Fi and Bluetooth identifiers remain unchanged.
The firmware build properties and local getprop report the same default locale.

- Teal/slate rounded cards, focus shapes, watch-face complication rings,
  quick settings, connectivity, Bluetooth, media, settings and About layouts
  based on the photos. Official Roboto coverage atlases replace the scaled 5 × 7
  font for normal text; text widths prevent long settings labels overlapping.
- Official Google Material Symbols Rounded replace the hand-drawn system
  icons, including Wi-Fi, Bluetooth, settings, gestures and media controls.
  [Icon provenance](src/material/symbols/SOURCE.md) and
  [font provenance](src/material/fonts/SOURCE.md) record the pinned sources
  and licenses. Both render offline from checked-in antialiased masks.
- The follow-up Wear references guide the black background, pastel cyan
  palette, round complication rings with real clock/connection data, large
  Roboto Light watch numerals, lavender/lime timer presets, scalloped media
  button and list launcher's Grid view footer. No health/weather values or
  song metadata are fabricated to imitate the reference photos.
- Bluetooth uses the existing TrouBLE HID host in the default build and
  flashing script. Pair new device opens a two-minute admission window;
  the watch shows the six-digit confirmation code. Select Confirm only when
  it matches the phone/PC. Disconnect and Forget devices are available.
  Bonds restore before the host starts and save in reserved ZPager slot 68.
  The raw-HCI discovery implementation remains a separate, mutually exclusive
  build (`radio-services` without `ble-hid`).
- Media: previous, play/pause, next, volume down/up dispatch HID Consumer
  Control reports to an encrypted, subscribed host. Media metadata and
  playback state are not supplied by HID; the UI shows connection readiness,
  not fabricated song titles or a playback state.
- Clock: working 5/10/15/30-minute timer presets and custom 1–60-minute duration
  editor with completion alert; pause/resume and reset
  stopwatch; daily alarm with hour/minute editor and dismissible alert.
  Alarm preferences persist in reserved slot 69. Timer and alarm evaluation
  continue while other pages are foreground. Alerts are visual because this
  board configuration has no audio or vibration hardware.
- Wi-Fi scan, crown password entry, connection and saved credentials retain
  the existing radio implementation. The Wi-Fi page also shows the actual
  acquired IPv4 address. SNTP and manual time remain the wall-clock sources.
- Physical crown input works in both PC-storage and SD modes. Button input
  has 25 ms debounce; release selects (<700 ms), goes Back (700–1799 ms), or
  Home (≥1800 ms), measured against the monotonic counter.
- Removed unused renderer helpers and the unused compiler feature. A local
  [HAL linker correction](ld/README.md) removes the spurious writable flag
  from the flash alignment reservation without suppressing warnings.

Assistant/phone notification forwarding, audio, sensors, a secure PIN lock,
and Google's apps are not implemented by this restoration. Existing gesture
and accessibility pages describe the available input/display behavior.
The current desktop viewer uses its own renderer; the preview below is
rendered from the actual firmware surface, not from that viewer.

## Built outputs

- [Firmware framebuffer preview](out/ui-preview/contact.png)
- [SD-mode application](out/watch-restored-sd.bin): application at **0x10000**.
- [PC-storage application](out/watch-restored-pc.bin): application at **0x10000**;
  requires the USB block server.
- [Merged SD-mode flash image](out/watch-restored-sd-full.bin): 4 MiB image at
  **0x0**, containing the ESP-IDF second-stage bootloader, partition table and
  application. This is a new build, not a copy of the recovered flash dump.
- [Fresh GPT userspace image](../Firmware/out/restored/aosp-wear-sd.img).
  This image has fresh userdata; writing the whole image over an existing SD
  card replaces that card's data. The existing image and recovery dump were
  not modified. Existing cards need the updated Rhai framework as well as the
  Rust application for the new menu actions.

No device or SD card was flashed as part of this work. For an application-only
update on an existing compatible ESP partition table:

```sh
espflash write-bin 0x10000 out/watch-restored-sd.bin --chip esp32s3
```

The logs `41/41` and `52/52` count transfer blocks of different images, which
may be compressed. They do not measure feature completeness. An application
write at 0x10000 leaves the lower bootloader/partition-table area intact;
a full image write starts at 0x0. Implementing UI features does not remove the
ESP32 ROM or the required second-stage bootloader.

## Verification

### Black ST7789 screen after a successful boot

`SYSTEM SERVER` / `system_server handoff succeeded` and changing `@ZWUI|M3`
screens show that the runtime and PC block storage are working. In `Storage: pc`
mode no SD-card write is required; keep the PC block server running.

The ST7789 driver now flushes SPI before changing DC or releasing CS. esp-hal
1.0.0 permits `SpiBus::write` to return before the last transfer finishes;
changing control pins early can make the panel discard its initialization
commands and leave it black. The regression test models a pending transfer
and checks initialization, commands, short pixel writes and a full UI frame.

Rebuild/reflash with `./flash`. If the panel remains black, run
`AOSP_WEAR_DISPLAY_DIAGNOSTIC=1 ./flash` to test the backlight and solid
red/green/blue fills at a slower SPI clock, independently of the UI. Physical
success still needs to be confirmed on the watch.

ST7789 SD/HID and default PC/HID release builds were checked for compiler and
linker warnings. Host checks execute the real Rhai runtime and rendering code,
substituting only the monotonic clock. They cover navigation/media/pairing
dispatch, timer presets/custom duration/expiry, stopwatch pause, alarm repeat/dismiss, button debounce
and hold duration, and all 32 screens × 16 cursors × 4 themes.

```sh
source ~/export-esp.sh
cargo +esp build --release
cargo +esp build --release --no-default-features --features rhai-runtime,display-st7789,ble-hid
cd tools/runtime-check
CARGO_ENCODED_RUSTFLAGS='' cargo +stable test --target x86_64-unknown-linux-gnu -- --test-threads=1
```

Host tests must run serially because the production RTC and clock state are
shared atomics. The empty host rustflags override prevents the embedded
`-nostartfiles` flag from leaking into the host test executable.

The fresh userspace image passes GPT/A-B metadata, boot-v4/CPIO, AVB SHA-256
descriptor and liblp-super checks. Physical display colors/orientation,
radio coexistence, phone HID compatibility and pairing across power cycles
still require hardware verification. Persisted bonds use the existing raw
development storage; this does not add flash encryption or production AVB.
