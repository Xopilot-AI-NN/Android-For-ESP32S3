#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
BOOTLOADER_DIR="${ZEPHYR_BOOTLOADER_DIR:-$ROOT/../Bootloader}"
OUT="$ROOT/out/target/product/zero"

cd "$ROOT"

echo '== AOSP Wear OS Firmware verify =='

python -m py_compile tools/build_firmware.py tools/inspect_super.py
bash -n build.sh

./build.sh
python tools/inspect_super.py "$OUT/super.img"

if [[ -f "$BOOTLOADER_DIR/tools/inspect_android_sd.py" ]]; then
  python "$BOOTLOADER_DIR/tools/inspect_android_sd.py" "$OUT/zephyr-watch-sd.img"
else
  echo 'NOTE: Bootloader image inspector not found; skipping whole-disk inspection.' >&2
fi

# Verify that the runtime components expected by Bootloader v1.4 exist in source.
for f in \
  ramdisk/init.rhai \
  system/framework/src/00_core.rhai \
  system/framework/src/10_servicemanager.rhai \
  system/framework/src/20_package_manager.rhai \
  system/framework/src/30_activity_manager.rhai \
  system/framework/src/40_input_manager.rhai \
  system/framework/src/50_surfaceflinger_systemui.rhai \
  system/framework/src/60_system_server.rhai \
  vendor/etc/zephyr/vendor_runtime.rhai \
  odm/etc/zephyr/odm_runtime.rhai \
  system_ext/etc/zephyr/system_ext_runtime.rhai \
  product/etc/zephyr/product_runtime.rhai
  do
    [[ -s "$f" ]] || { echo "Missing runtime component: $f" >&2; exit 1; }
  done

if [[ -x "$BOOTLOADER_DIR/verify.sh" ]]; then
  echo
  echo '== Bootloader verify =='
  "$BOOTLOADER_DIR/verify.sh"
fi

echo
echo 'OK: Firmware source, Android boot images, GPT, AVB descriptors and liblp super verified.'
