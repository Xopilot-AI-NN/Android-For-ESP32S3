# AOSP Wear OS Firmware 0.7

AOSP/Wear-shaped Rhai userspace for ESP32-S3-Zero-N4R2.

The build emits Android-style boot images, GPT/A-B metadata, AVB development descriptors,
liblp `super`, system/vendor/product/odm/system_ext logical partitions, and the complete
raw disk image used by both microSD and USB-PC block boot.

The runtime includes an expanded Wear Material 3 Expressive SystemUI, launcher, notifications,
Quick Settings, Settings/Display/Connectivity/System pages, widgets, media and clock surfaces.
Native Wi-Fi/BLE and wireless ADB are driven by the Rust boot/runtime layer.

Build and verify:

```bash
./build.sh
./verify.sh
```

Output is written to `out/target/product/zero/`.
