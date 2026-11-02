# AOSP Wear OS 17.1.5 — thermal + full keyboard

17.1.5 is a wearable-thermal and Wi-Fi input polish release for the ESP32-S3-Zero-N4R2 target.

## Wearable thermal profile

- CPU preset is reduced from the chip maximum to 80 MHz. esp-radio requires a minimum CPU clock of 80 MHz, so this is the lowest supported fixed-clock wearable profile while Wi-Fi remains available.
- Wi-Fi uses `PowerSaveMode::Minimum`, allowing the modem/PHY to sleep between DTIM beacons while remaining usable for DHCP, SNTP and wireless ADB.
- Wi-Fi and BLE still shut down completely when disabled in SystemUI.
- Active Wi-Fi scanning remains explicit (`Available networks`) rather than background polling.

This firmware policy reduces heat generation, but it cannot make unsafe mechanical placement safe. Do not cover the ESP32-S3-Zero ceramic antenna with a heatsink or other material.

## Full Wi-Fi password keyboard

The old one-character carousel is replaced by a visible QWERTY keyboard:

- lowercase page with QWERTY rows;
- Shift/uppercase page;
- `123` numbers/symbols page plus a second `#+=` symbol page;
- all printable ASCII characters used by normal WPA/WPA2 passphrases are selectable;
- Space, Delete, Connect and Back keys;
- crown rotation moves the selected key;
- crown press activates it;
- long crown press keeps the Connect shortcut.

The physical 5x7 font now contains lowercase letters and the punctuation used by the keyboard. Password contents remain local to the watch runtime; the desktop viewer receives only password length plus keyboard page/selection state.
