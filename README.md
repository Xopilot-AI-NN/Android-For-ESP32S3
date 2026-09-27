# Zephyr Android Firmware 0.6

AOSP-like Rhai userspace for **Zephyr Watch / ESP32-S3-Zero-N4R2**.

Current build includes Android-style GPT/A-B images, AVB development verification,
liblp `super`, Rhai system services, ZPager, native Material 3 Expressive SystemUI,
Wi-Fi/BLE connectivity and wireless ADB bring-up.

Build and verify:

```bash
./build.sh
./verify.sh
```

Output is written to `out/target/product/zero/`.
