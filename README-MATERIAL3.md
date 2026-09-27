# AOSP-ESP32S3 v0.2 Material UI DevKit

This devkit combines **Bootloader 1.5.0** and **Zephyr Android Firmware
0.2.0-dev**.

The system still boots through GPT -> A/B -> AVB -> Android boot images ->
liblp `super`, but SystemUI now renders as a native RGB565 Material-style UI
instead of the old four-line diagnostic surface.

## Build and run

```bash
cd Firmware
./build.sh
./verify.sh

cd ../Bootloader
cargo clean
./flash.sh st7789 pc
```

Keep the block server terminal open. In another terminal:

```bash
cd Bootloader
python desktop_viewer.py
```

Expected boot markers include:

```text
Zephyr Watch bootloader 1.5.0
slot=a version=0.2.0-dev
MOUNTING SUPER
SYSTEM SERVER
system_server handoff succeeded
```

The first SystemUI screen should be a green Material home surface. Crown press
opens Apps. Settings -> Theme cycles green, blue, purple and coral.

See `Bootloader/MATERIAL_UI.md` and `Bootloader/RHAI_STATE_ABI.md`.
