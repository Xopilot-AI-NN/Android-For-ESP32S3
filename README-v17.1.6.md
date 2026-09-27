# AOSP Wear OS 17.1.6

17.1.6 is a radio-stability hotfix over 17.1.5.

The 17.1.5 thermal experiment lowered the ESP32-S3 CPU to 80 MHz and enabled modem power saving before station bring-up. On the USB-PC block build this can starve the shared USB/radio runtime when Wi-Fi is enabled and make the watch appear frozen.

17.1.6 restores the proven transport-safe CPU/radio bring-up path from 17.1.4 while keeping the full QWERTY Wi-Fi keyboard, software RTC/SNTP, in-watch Wi-Fi setup, persistent settings, wireless ADB and all later UI work.

The board still fully stops Wi-Fi when it is disabled and does not background-scan. Thermal reduction will be reintroduced only as a post-association/adaptive policy after it can be validated without destabilizing USB-PC storage.
