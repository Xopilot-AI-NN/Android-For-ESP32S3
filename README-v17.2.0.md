# AOSP Wear for ESP32-S3 — 17.2.0

17.2.0 is the first UI-overhaul release built specifically around the physical hardware contract of this project: **one rotary encoder with one integrated push button, no touchscreen**.

## Launcher

The launcher now has two Wear-style modes:

- **Grid view** — honeycomb/bubble layout with six system apps and a focused icon that grows as the crown moves.
- **List view** — expressive pill cards with icon + label and a visible `GRID VIEW` switch.

Apps exposed by the launcher are Clock, Media, Settings, Connectivity, System Info and Assistant. Notifications, Quick Settings and Tiles remain system surfaces rather than fake launcher apps.

## Encoder-only navigation

Hardware navigation is now deterministic:

- rotate crown: move focus / scroll;
- short press: activate focused item;
- long press (~0.7 s): Back;
- very long press (~1.8 s): Home / watch face.

Touch/swipe controls remain only in the desktop viewer for development/testing.

## Wi-Fi keyboard

The password editor was redrawn as a compact phone/Gboard-like keyboard without a suggestion strip. It keeps full QWERTY, upper/lower case, number and symbol pages, while making the selected key visibly lift and enlarge for encoder use.

The keyboard still keeps the password private in the PC viewer transport.

## Assistant shell

A new service-neutral Wear-style Assistant surface is included as a UI/companion integration point. It intentionally does **not** claim a Google/Gemini backend. The future phone companion can provide the actual assistant transport.

## Preserved 17.1.x work

- watch-side Wi-Fi provisioning;
- software RTC + Wi-Fi SNTP correction;
- persistent settings and Wi-Fi credentials;
- wireless ADB;
- transport-safe Wi-Fi/USB-PC block profile;
- Material 3 / dark Wear visual system.
