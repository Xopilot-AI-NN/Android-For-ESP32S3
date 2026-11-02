# AOSP Wear OS 17.2.1

Android identity remains **Android 17 QPR1 / API 37**. `17.2.1` is the project release number.

## Changes

- ESP32-S3 runs at `CpuClock::max()`; on ESP32-S3 this is the 240 MHz maximum.
- Wi-Fi credentials typed with the crown are treated as an unverified candidate until association succeeds. A typo no longer replaces the saved working profile.
- The physical ST7789 Wi-Fi editor displays the entered password tail in clear text so encoder overshoot is visible before `CONNECT`. Remote protocols do not export those bytes.
- A browser remote is hosted directly by the watch on TCP/80 after Wi-Fi reaches DHCP Online. Open `http://<watch-ip>/` from a phone/laptop on the same network.
- Browser controls map to the same runtime events as the encoder: crown left/right, press, Back, Home and Wear swipe events.

## Browser remote

After connection, the log prints:

```text
web: remote UI listening on http://10.x.x.x/
```

The same URL is included by `python Bootloader/zadb.py wifi-status`.

No cloud relay is used. The first implementation is intentionally LAN-only and unauthenticated, so use it on a trusted hotspot/LAN.
