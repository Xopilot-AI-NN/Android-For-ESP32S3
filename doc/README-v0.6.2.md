# Zephyr Android 0.6.2

Wi-Fi stability hotfix for ESP32-S3.

The connection path now keeps the station driver alive across normal association retries, performs unfiltered all-channel scans and matches the requested SSID locally, and only does a hard station reset after three consecutive timeouts.
