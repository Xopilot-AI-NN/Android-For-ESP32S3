# Wear Material UI ABI — release 17.2.1

The physical SystemUI renderer is native Rust/RGB565. Rhai owns Activity navigation and compact
framework state. The desktop viewer mirrors the semantic frame and can inject the same input events.

## Runtime scene ABI

Rhai → Rust:

```text
M3|screen|cursor|brightness|dnd|airplane|theme|locked|wifi|bt|adb|notes|time_minutes|timer_secs|stopwatch_secs|alarm|auto_time|time_valid|timezone_hours
```

Rust → PC viewer adds `swap_pages` after `time_minutes`:

```text
@ZWUI|M3|screen|cursor|brightness|dnd|airplane|theme|locked|wifi|bt|adb|notes|time_minutes|swap_pages|timer_secs|stopwatch_secs|alarm|auto_time|time_valid|timezone_hours
```

`Sync` requests the current scene only; it never changes wall-clock time.

## Screen IDs

```text
 0 lock                12 Wi-Fi
 1 watch face          13 Bluetooth
 2 grid launcher       14 Display
 3 notifications       15 System
 4 Quick Settings      16 Date & time
 5 Settings            17 Sound & vibration
 6 Clock               18 Gestures
 7 Connectivity        19 Accessibility
 8 About               20 Security
 9 Tiles               21 Apps & notifications
10 Media controls      22 Developer options
11 Clock tools         23 Available networks
                       24 Wi-Fi keyboard
                       25 Wi-Fi connection
                       26 list launcher
                       27 Assistant
```

## Input contract

Production hardware is encoder-only:

```text
rotate crown          focus / scroll
short press           select
long press ~0.7 s     Back
very long ~1.8 s      Home / watch face
```

The desktop development viewer may still inject `POWER`, `ROTATE`, `SWIPE`, `NORMAL` and `SYNC`
events. Swipe is a test-only convenience and is not required by the watch UI.

## Visual rules

- true black base surface;
- dynamic primary/secondary container hierarchy;
- large numeral role for glanceable time;
- expressive selected rows that expand toward display edges;
- pill / rounded quick-setting controls;
- no developer/USB status badges on normal watch surfaces;
- Wireless ADB appears only in Developer options.

## Wi-Fi provisioning extension

The base `M3` state is followed by optional watch-side provisioning fields:

`ap_count | ap_index | ap_rssi | password_len | editor_char | link_state | ssid_hex`

Screens:

- `23` Available networks
- `24` Password editor
- `25` Connection progress

This extension is filled by the native runtime after Rhai renders the base scene, so credentials never enter the Rhai framework state or the viewer protocol as clear text. Only password length is mirrored to the development viewer.


## 17.2.1 keyboard encoding

On screen 24 the existing `editor_char` byte is packed as `page<<6 | selected_key`. Pages are 0=lowercase QWERTY, 1=uppercase QWERTY, 2=numbers/symbols, 3=remaining printable ASCII symbols. This avoids exposing password contents or changing the USB viewer frame shape.
