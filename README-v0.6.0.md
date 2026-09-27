# Zephyr Watch v0.6.0

Bootloader 1.9.0 + Firmware 0.6.1-dev.

Main focus: reliable wireless ADB shell streams on top of the already working
Wi-Fi/DHCP transport.

## Highlights
- `adb devices -l` identity: Zephyr Watch / zephyr / Xopilot.
- `adb shell getprop ...` one-shot commands close correctly.
- Interactive `adb shell` with prompt and `exit`.
- Atomic ADB packet queueing; no partial header/payload writes.
- Correct ADB CLOSE semantics (recipient does not echo CLOSE).
- WADB TCP_NODELAY-style behaviour via disabled Nagle + keepalive/timeout.
- Firmware build properties bumped to 0.6.1-dev.

## Flash
From repository root:

```bash
./flash
```

## Smoke test

```bash
adb connect WATCH_IP:5555
adb devices -l
adb shell getprop ro.product.model
adb shell id
adb shell wm size
adb shell
```
