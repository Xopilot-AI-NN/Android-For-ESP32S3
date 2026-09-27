# AOSP Wear for ESP32-S3 — release 17.1.5

17.1.5 is a connectivity/USB-block reliability hotfix on top of 17.1.3.

## Wi-Fi no longer kills the PC block transport

The ESP32-S3 native USB-Serial-JTAG endpoint can temporarily apply host-side
backpressure while `esp-radio` performs an active Wi-Fi scan. The old PC block
server used a five-second `pyserial` write timeout. If a block reply happened to
meet that radio stall, `SerialTimeoutException` terminated
`usb_block_server.py`; the watch then appeared frozen because its GPT/ZPager
backing device had disappeared.

17.1.5 changes the PC block data path to framing-safe blocking writes. Temporary
USB backpressure is therefore treated as flow control rather than as a fatal
storage disconnect. The ESP side still has its own 30-second block-protocol
watchdog.

Wi-Fi enable itself also no longer performs a synchronous active scan. Turning
Wi-Fi on only starts the STA and reconnects a saved profile. A scan happens when
`Settings → Connectivity → Wi-Fi → Networks` is opened, where a short scan pause
is expected and visible to the user.

## Software RTC

The 17.1.3 RTC model remains unchanged:

- the clock ticks locally from the ESP monotonic counter even with Wi-Fi off;
- an unsynchronised boot still renders digits starting from `00:00` and marks
  the time as **SYNCING TIME**;
- last-known valid local time is restored from ZPager holdover;
- when DHCP becomes online, SNTP corrects the software RTC;
- network time is refreshed periodically while Automatic time is enabled;
- USB/viewer time is never injected automatically.

## Validation

Run the normal one-shot flow:

```bash
./flash
```

To test the regression specifically:

1. boot to the watch face;
2. turn Wi-Fi on in the watch UI;
3. enter `Networks` and wait for AP discovery;
4. return to the watch face;
5. verify `usb_block_server.py` is still alive and the clock continues ticking;
6. after DHCP, look for `time: NTP sync received ...` and
   `time: software RTC synchronized from Wi-Fi SNTP`.
