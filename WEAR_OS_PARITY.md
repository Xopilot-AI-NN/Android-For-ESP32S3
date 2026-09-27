# Wear OS parity boundary — release 17.1.6

This project deliberately separates **behavioral/design parity** from capabilities that cannot exist
on an ESP32-S3. It does not claim to be Google's proprietary Wear OS image.

## Native on the ESP32-S3

- GPT, A/B boot metadata, Android boot-image v4 parsing and AVB hash verification.
- Android liblp `super` parsing and logical partitions.
- Native ST7789 RGB565 SystemUI compositor in PSRAM.
- Wear-shaped Activity/Package/Service framework implemented in bounded Rhai.
- Material 3 Expressive-style watch face, launcher, notifications, Quick Settings, Tiles and Settings.
- Crown navigation and Wear gesture events; the PC viewer can inject swipes for touch-parity testing.
- 2.4 GHz Wi-Fi station mode, scan, saved profile, DHCP and reconnect.
- BLE advertising / scanning / LE connection path.
- Wireless ADB transport on TCP/5555 when explicitly enabled in Developer options.
- Timer, stopwatch and alarm state.
- 32 MiB ZPager backing store for explicit serialized Activity state.

## Time parity

- Automatic time does **not** use the development PC as a hidden clock source.
- Automatic date & time waits for an explicit companion source, matching the paired-watch model.
- Manual adjustment is available when automatic time is disabled.
- Once set, time advances from the ESP monotonic clock for the current powered session.
- A full power loss cannot preserve absolute time on this board without an always-powered RTC or a
  paired companion update after boot. The firmware reports that limitation instead of inventing time.

## Hardware-adapted surfaces

The target development panel is 240×280 and rectangular, while production Wear watches are commonly
round. Components therefore preserve Wear hierarchy, black canvas, edge-oriented containers and
expressive selection states but are geometrically adapted to the actual panel.

The board has a crown/encoder and no touch controller. Swipe events exist in the UI contract and PC
viewer, while crown rotation/press provides a deterministic physical-board fallback.

## Intentionally unavailable instead of faked

- Google Play Services, Play Store, Wallet, Gemini, Fitbit proprietary services.
- APK/DEX/ART execution, Android Linux kernel and Binder.
- NFC, GNSS, cellular/eSIM, speaker, microphone, haptics or sensors not physically installed.
- DRM / Play Integrity certification.
- Media control of a phone until a real companion media transport is implemented.

The rule for 17.1.6 is: if ESP32-S3 can implement a Wear behavior, implement it; if hardware or Google
proprietary infrastructure is missing, expose the limitation instead of fabricating a working state.

## Connectivity/time parity

Normal Wi-Fi provisioning no longer requires the development bridge. The watch scans APs, accepts credentials, associates, obtains DHCP, persists the profile and exposes connection progress in SystemUI.

Wall time is owned by a software RTC service. Network availability corrects the RTC through SNTP; network loss does not stop the clock. USB viewer time injection remains prohibited.

### Thermal/input parity

Wi-Fi credential entry uses a visible QWERTY keyboard surface rather than a development-style character carousel. The experimental 80 MHz/pre-association modem-power-save profile from 17.1.5 is not enabled by default because it can destabilize the shared USB-PC-block/radio runtime; 17.1.6 uses the proven transport-safe bring-up path.
