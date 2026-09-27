# AOSP Wear for ESP32-S3 — current release 17.2.1

**Platform identity:** Android 17 QPR1 / API 37  
**Project release:** 17.2.1  
**Target:** Waveshare ESP32-S3-Zero-N4R2 + ST7789V3 240×280

17.2.1 keeps the encoder-first Wear UI from 17.2.0, fixes Wi-Fi credential handling and adds a browser remote served directly by the ESP32-S3.

```bash
./flash
```

After Wi-Fi is online, open `http://<watch-ip>/` from a phone or laptop on the same LAN/hotspot. The URL is also shown by `wifi-status`.

See [`README-v17.2.1.md`](README-v17.2.1.md) and [`WEAR_OS_PARITY.md`](WEAR_OS_PARITY.md).
