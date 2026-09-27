# Project release 17.2.1 Wi-Fi / browser remote

- ESP32-S3 maximum CPU clock retained (240 MHz on ESP32-S3 via `CpuClock::max()`).
- Wi-Fi credentials are committed only after successful association.
- Physical Wi-Fi password entry now shows the actual entered tail to catch encoder overshoot.
- Added an ESP-hosted HTTP remote at `http://<watch-ip>/` with crown, Back/Home and swipe controls.

# Project release 17.2.1 radio stability hotfix

- Fixed Wi-Fi-enable freezes caused by the 17.1.5 80 MHz/pre-association power-save experiment.
- Restored the proven transport-safe CPU/radio bring-up while retaining the full QWERTY Wi-Fi keyboard.

# Project release 17.1.5 thermal/input polish

- Reduced the fixed ESP32-S3 CPU clock to the 80 MHz radio-safe minimum for this wearable build.
- Enabled DTIM-aware minimum modem power saving while Wi-Fi is connected.
- Replaced the crown character carousel with a visible four-page keyboard covering printable ASCII passphrase characters.
- Added lowercase and punctuation glyphs to the native ST7789 renderer while keeping password bytes private.

# Project release 17.1.4 reliability hotfix

- Fixed a PC-block transport crash triggered by USB backpressure during ESP32-S3 Wi-Fi radio work.
- `usb_block_server.py` now uses blocking, complete writes so transient radio stalls cannot tear down the GPT/ZPager backing disk.
- Wi-Fi enable is non-scanning; AP discovery runs only when the user opens the network list.
- Software RTC continues independently and is corrected by SNTP after DHCP reaches Online.

# Android 17 QPR1 / Wear-parity changes

## Project release 17.1.3

- Material 3 Expressive-style Wear system surfaces and gesture hierarchy.
- No implicit PC/USB wall-clock injection.
- Explicit companion/manual time model.
- Quick Settings, Tiles, notifications, launcher and expanded Wear Settings hierarchy.
- Wireless debugging moved under Developer options.
- Package surface renamed from Widgets to Tiles.
- Project release identity moved to 17.1.3 while Android platform identity remains 17/QPR1/API 37.

# Android 17 QPR1 / AOSP Wear OS update

This patch keeps the ESP32-S3/Rhai architecture but changes the externally visible
Android identity and SystemUI presentation.

## Networking

- DHCP client sends option 12 hostname: `android-aosp-wear`.
- Wireless ADB accepts the checksum-less packets used by ADB protocol 0x01000001,
  fixing modern `adb shell` / `adb shell getprop` packet checksum mismatches.
- ADB identity is now `product:aosp_wear model:AOSP Wear OS device:aosp_wear`.

## Android identity

- Android release: `17`
- SDK/API: `37`
- Display ID: `AOSP Wear OS Android 17 QPR1`
- Build type: `userdebug`
- Hostname: `android-aosp-wear`
- BLE device name: `AOSP Wear OS`

This is an AOSP-style compatibility identity for this ESP32-S3 runtime; it does
not turn the board into Google's certified Wear OS distribution.

## UI

The ST7789 renderer and desktop viewer now use a Wear OS-style Material 3 layout:
large watch-face clock, compact status chrome, complications, round quick-setting
buttons, curved/expanded list selection, Android System notifications, and a
System/About screen identifying Android 17 QPR1 / API 37.

## Validation

`Firmware/verify.sh` passes for the regenerated boot images, GPT, AVB descriptors,
liblp super image and Rhai runtime bundle. Rust/ESP compilation still needs to be
run on a machine with the project's Rust toolchain installed.

## v0.7.0 full shell pass

- Bootloader package version advanced to 2.0.0; firmware to 0.7.0-dev.
- Product disk/viewer names changed to `aosp-wear-*`.
- Runtime scene ABI accepts 16 system surfaces instead of 9.
- Crown Home semantics now return to the watch face instead of re-locking.
- Launcher expanded to Notifications, Widgets, Media, Clock, Settings,
  Connectivity, System and About.
- Added Widgets, Media, Clock Tools, Wi-Fi details, Bluetooth details,
  Display and System surfaces.
- Settings navigation now routes into real Wi-Fi/BLE/WADB state bits, so
  native `RadioServices::sync_frame()` still drives the hardware radios.
- Notification stream can dismiss its embedded notifications.
- Wear service registry expanded with notification, media_session, alarm,
  display, battery, launcher and wearable services.
- ADB shell adds watch feature declarations and more useful dumpsys targets.
- Android properties add `ro.build.characteristics=watch`, API/security patch
  fields, and Wear runtime identity.
- System package tree now contains Wear Launcher, Widgets, Media and
  Connectivity package manifests.

- Host local time is synchronized into SystemUI through the USB block/viewer bridge.
- Crown short press selects; ~800 ms hold is an always-available Home gesture.
- Clock Tools now run a 5-minute timer and stopwatch in the native runtime and refresh once per second.
- Active liblp runtime component paths were renamed from the old project codename to `aosp_wear`.
- Primary build environment variables use `AOSP_WEAR_*`, while old `ZEPHYR_*` names remain compatibility aliases.

### 17.1.3 reliability / UI pass

- Wireless debugging now drives the correct ADB state bit and persists across reboots.
- ZPager metadata is isolated from Activity page slots.
- Last-known clock state is retained without reintroducing USB-host time injection.
- Launcher and settings icons use native vector primitives instead of letter placeholders.

## 17.1.3 — watch-side Wi-Fi + software RTC

- Added native Wi-Fi network selection and password entry from the crown-only Wear UI.
- Added a dedicated software RTC module independent of the USB viewer.
- Added automatic SNTP correction after Wi-Fi DHCP becomes online, retry/failover, and six-hour resync.
- Removed `--:--` from normal watch surfaces: an unsynchronised software clock ticks locally and is visibly marked as syncing.
- Persisted RTC automatic/manual mode and timezone separately from Activity state.

## 17.2.1

- Fixed a regression where enabling Wi-Fi could freeze the watch on USB-PC block builds.
- Reverted the experimental 80 MHz CPU clock used by 17.1.5 to the previously stable maximum clock while the shared USB/radio runtime is active.
- Removed pre-association modem power-save configuration from Wi-Fi initialization; radio bring-up now follows the proven 17.1.4 path.
- Kept the 17.1.5 full QWERTY password keyboard and 17.1.3+ software RTC/SNTP behavior unchanged.

## 17.2.1 UI adaptation

- Wear-style bubble/grid and list launcher modes.
- Encoder-first Back/Home semantics; no touchscreen dependency.
- Phone-keyboard visual adaptation for Wi-Fi password entry.
- Assistant surface reserved for a future companion application.
