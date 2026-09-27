# AOSP-ESP32S3 / Zephyr Watch v0.3.1 fixed devkit

Hotfix over v0.3.0:

- fixes the missing direct `heapless` dependency used by desktop/ZADB formatting;
- default Bootloader features now include ST7789 + PC block + Rhai + native radio services;
- `./flash` is the normal development entry point: it builds Firmware, builds the Bootloader,
  flashes the ESP32-S3, starts the PC block server and opens the live viewer when Tk is available;
- no positional flags are required;
- avoids the old X11 `$DISPLAY` variable collision by using `ZEPHYR_DISPLAY` only for overrides;
- cleans PC-mode-only unused imports and the `PostInit` naming warning.

Normal use from the repository root:

```bash
./flash
```

Or from `Bootloader/`:

```bash
./flash
```

Optional escape hatches (not required):

```bash
ZEPHYR_SKIP_FIRMWARE_BUILD=1 ./flash
ZEPHYR_DISPLAY=ssd1306 ./flash
ZEPHYR_STORAGE=sd ZEPHYR_RADIO=off ./flash
```

Default feature set:

```text
rhai-runtime,display-st7789,pc-block-boot,radio-services
```
