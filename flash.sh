#!/usr/bin/env bash
set -euo pipefail

if [[ -f "$HOME/export-esp.sh" ]]; then
  # shellcheck disable=SC1090
  source "$HOME/export-esp.sh"
fi

case "${DISPLAY:-st7789}" in
  ssd1306)
    FLAGS=(--no-default-features --features "rhai-runtime,display-ssd1306")
    ;;
  st7789)
    FLAGS=(--no-default-features --features "rhai-runtime,display-st7789")
    ;;
  *)
    echo "DISPLAY must be ssd1306 or st7789" >&2
    exit 2
    ;;
esac

echo "Building Zephyr Watch bootloader for DISPLAY=${DISPLAY:-st7789}"
cargo +esp build --release "${FLAGS[@]}"
cargo espflash flash --release --monitor --chip esp32s3 "${FLAGS[@]}"
