# AOSP Wear for ESP32-S3 — current release 17.1.6

**Platform identity:** Android 17 QPR1 / API 37  
**Project release:** 17.1.6  
**Target:** Waveshare ESP32-S3-Zero-N4R2 + ST7789V3 240×280

17.1.6 is a radio-stability hotfix for a regression introduced by the 17.1.5 thermal experiment. Running the shared USB-PC-block + Wi-Fi runtime at 80 MHz and enabling modem power save before station bring-up could make the watch appear frozen as soon as Wi-Fi was enabled.

The release restores the proven radio/USB bring-up profile while keeping the full QWERTY Wi-Fi keyboard, watch-side Wi-Fi provisioning, software RTC/SNTP, persistent settings and wireless ADB.

See [`README-v17.1.6.md`](README-v17.1.6.md) and [`WEAR_OS_PARITY.md`](WEAR_OS_PARITY.md).

## Build / flash

```bash
./flash
```

## Thermal note

The aggressive 80 MHz default is disabled in 17.1.6 because stability takes priority on the USB-PC storage build. Wi-Fi still fully stops when disabled and no background scan runs. Further thermal work should use adaptive/post-association policies rather than reducing the entire runtime clock during radio bring-up.
