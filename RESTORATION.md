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
  Roboto Light watch numerals, theme-aware timer presets, scalloped media
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
- Crown rotation uses ESP32-S3 PCNT quadrature counting with a hardware glitch
  filter, so SPI rendering no longer loses intermediate edges. Settings →
  Gestures provides low/normal/high sensitivity (4/2/2 edges per UI step) and
  reverse direction. Accessibility provides a working high-contrast option.
  Normal is as responsive per edge as the former Fast mode; Normal emits at
  most one step per loop and Fast at most two. Large movements drain smoothly;
  reversing cancels unrendered motion rather than replaying stale steps.
  Preferences persist in reserved page 71.
- ST7789 BLK/GPIO6 uses LEDC hardware PWM at 80 kHz, 8-bit resolution. The
  brightness editor changes the actual duty (5–100%). Flashlight temporarily
  uses 100% and restores the selected brightness on exit. Media, timer, tiles
  and watch face derive colors from the selected theme. Identical brightness
  values no longer restart PWM on every frame. Actual panel flicker/camera
  banding still needs an on-device check.
- MADCTL now selects BGR (`0x08`) for the user's panel: the reported coral/blue
  swap with unchanged purple indicates exchanged red and blue channels.
  RGB565 framebuffer values remain unchanged; conversion is in the controller.
- The app launcher opens as a list. The optional grid has aligned rows instead
  of staggered circles. Settings and the app list share one proportional edge
  scroll indicator; the misleading marker inside every selected card is gone.
- Display → Screen sleep selects 30 seconds or 1–10 minutes (the same timeout
  as Security → Auto-lock). Idle screen-off works without a PIN: backlight duty
  is zero and DISPON is disabled. The first crown action wakes only; the new
  frame is transferred before the light comes on. Timer/alarm alerts wake the
  display. The ESP32 keeps running timers/radio/storage; this is screen sleep,
  not complete power-off or deep sleep.
- Security provides four-digit PIN enrollment with confirmation, current-PIN
  verification to change/disable, immediate locking and an idle timeout of
  30 seconds or 1–10 minutes. PIN entry uses crown rotation and press; Back
  erases a digit. A configured PIN is required again after reboot. Five wrong
  attempts impose a 30-second delay; the attempt state survives normal restarts.
  Only a salted, iterated SHA-256 verifier is stored (page 70), never the PIN
  digits. UI actions, Home and state replacement cannot dismiss the PIN gate.
  New settings show a pending-save status until storage confirms the write.
  Timer/alarm alerts remain visible while locked; dismissing an alert does not
  unlock the watch. Auto-lock activates after a PIN has been enrolled and saved.
- Removed unused renderer helpers and the unused compiler feature. A local
  [HAL linker correction](ld/README.md) removes the spurious writable flag
  from the flash alignment reservation without suppressing warnings.

Assistant/phone notification forwarding, audio, sensors and Google's apps are
not implemented by this restoration. PIN is a local UI lock, not a hardware
anti-theft/activation lock: this development board does not have configured
secure boot, flash encryption or protected internal credential storage. Removing
or replacing the external disk, modifying its contents or reflashing can bypass
it. Keep a private backup of the disk; forgetting the PIN requires recovery.
The current desktop viewer uses its own renderer; the preview below is
rendered from the actual firmware surface, not from that viewer.

### Bluetooth diagnosis

The running watch was detected by a real laptop BLE scan as `AOSP Wear`.
Its radio was advertising; phone pairing/media operation
has not yet been confirmed. Entering Pair new device or explicitly enabling
Bluetooth from the watch now opens a 120-second pairing window immediately.
Startup, actual advertising readiness, pairing and errors have separate status
codes and logs. The no-reset `zadb.py bt-status` command additionally reports
remaining pairing time and internal heap. A fatal host/startup timeout reports
that a reboot is needed instead of silently waiting forever.

This remains BLE HID Consumer Control, not Bluetooth Classic audio or a Google
Wear OS companion service. If phone pairing still fails after the update,
the phone model/OS and the new `bt-status` plus `BLE:` logs are needed to trace it.

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

Rebuilding an existing compatible PC image now validates its GPT layout and
preserves `metadata`, `swap` (PIN/settings/bonds) and `userdata`. System images
are replaced atomically. A layout/size mismatch or failed build leaves the old
disk intact. Stop the block server before rebuilding: POSIX file locks prevent
replacing an actively served image. A new output path still produces a fresh
disk. Writing that fresh whole-disk image onto a physical card erases its data.

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
