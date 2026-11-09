#!/usr/bin/env bash
# Модуль проверок проекта.
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

python tools/check_board_wiring.py

rm -rf out/verify
mkdir -p out/verify

python -m py_compile \
  desktop_viewer.py \
  fastboot_tool.py \
  usb_block_server.py \
  zadb.py \
  tools/mk_android_sd.py \
  tools/inspect_android_sd.py
bash -n flash.sh
python tools/mk_android_sd.py \
  --output out/verify/aosp-wear-sd.img \
  --size-mib 256 \
  --version verify
python tools/inspect_android_sd.py out/verify/aosp-wear-sd.img

if command -v cargo >/dev/null 2>&1; then
  if cargo +esp --version >/dev/null 2>&1; then
    cargo +esp fmt --manifest-path "$ROOT/../Firmware/Cargo.toml" --check
    cargo +esp build --manifest-path "$ROOT/../Firmware/Cargo.toml" --release --no-default-features --features 'rhai-runtime,display-st7789'
    cargo +esp build --manifest-path "$ROOT/../Firmware/Cargo.toml" --release --no-default-features --features 'rhai-runtime,display-ssd1306'
    cargo +esp build --manifest-path "$ROOT/../Firmware/Cargo.toml" --release --no-default-features --features 'rhai-runtime,display-st7789,pc-block-boot'
    cargo +esp build --manifest-path "$ROOT/../Firmware/Cargo.toml" --release --no-default-features --features 'rhai-runtime,display-ssd1306,pc-block-boot'
    if [[ "${AOSP_WEAR_VERIFY_RADIO:-${ZEPHYR_VERIFY_RADIO:-0}}" == "1" ]]; then
      cargo +esp build --manifest-path "$ROOT/../Firmware/Cargo.toml" --release --no-default-features --features 'rhai-runtime,display-st7789,pc-block-boot,radio-services'
    fi
  else
    echo 'NOTE: cargo exists but +esp toolchain is unavailable; skipping Rust/ESP build.' >&2
  fi
else
  echo 'NOTE: cargo is unavailable; skipping Rust/ESP build.' >&2
fi
