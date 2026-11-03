#!/usr/bin/env bash
# Собирает отдельные flash-сегменты загрузчика, таблицы разделов и прошивки без записи устройства.
set -Eeuo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ELF="${1:-$ROOT/../Firmware/target/xtensa-esp32s3-none-elf/release/firmware}"
OUTPUT="${2:-$ROOT/out/flash-bundle}"
ESPFLASH_BIN="${ESPFLASH_BIN:-$(command -v espflash || true)}"
if [[ -z "$ESPFLASH_BIN" && -x "$HOME/.cargo/bin/espflash" ]]; then
  ESPFLASH_BIN="$HOME/.cargo/bin/espflash"
fi
[[ -f "$ELF" ]] || { echo "Missing firmware ELF: $ELF" >&2; exit 1; }
[[ -n "$ESPFLASH_BIN" ]] || { echo "espflash is required" >&2; exit 1; }
mkdir -p "$OUTPUT"
"$ESPFLASH_BIN" save-image --skip-update-check --chip esp32s3 --flash-size 4mb --merge --skip-padding \
  --partition-table "$ROOT/partitions.csv" "$ELF" "$OUTPUT/flash.bin"
node "$ROOT/tools/split_flash.cjs" "$OUTPUT/flash.bin" "$OUTPUT"
