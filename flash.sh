#!/usr/bin/env bash
set -Eeuo pipefail

BOOTLOADER_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
FIRMWARE_DIR="${ZEPHYR_FIRMWARE_DIR:-$BOOTLOADER_DIR/../Firmware}"
cd "$BOOTLOADER_DIR"

if [[ -f "$HOME/export-esp.sh" ]]; then
  # shellcheck disable=SC1090
  source "$HOME/export-esp.sh"
fi

# v0.4 developer default: one command means the complete configuration.
# No flags are required:
#   ST7789 + PC block storage + Rhai userspace + native Wi-Fi/BLE radio.
# Environment variables remain available only as escape hatches.
DISPLAY_NAME="${ZEPHYR_DISPLAY:-st7789}"
STORAGE_NAME="${ZEPHYR_STORAGE:-pc}"
RADIO_NAME="${ZEPHYR_RADIO:-radio}"

case "$DISPLAY_NAME" in
  ssd1306|st7789) ;;
  *) echo "ZEPHYR_DISPLAY must be ssd1306 or st7789" >&2; exit 2 ;;
esac
case "$STORAGE_NAME" in
  sd|pc) ;;
  *) echo "ZEPHYR_STORAGE must be sd or pc" >&2; exit 2 ;;
esac
case "$RADIO_NAME" in
  off|radio) ;;
  *) echo "ZEPHYR_RADIO must be off or radio" >&2; exit 2 ;;
esac

FEATURES="rhai-runtime,display-${DISPLAY_NAME}"
[[ "$STORAGE_NAME" == "pc" ]] && FEATURES+=",pc-block-boot"
[[ "$RADIO_NAME" == "radio" ]] && FEATURES+=",radio-services"
FLAGS=(--no-default-features --features "$FEATURES")

BOOT_VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n1)"
echo "== Zephyr Watch one-shot flash =="
echo "Bootloader : v${BOOT_VERSION}"
echo "Display    : ${DISPLAY_NAME}"
echo "Storage    : ${STORAGE_NAME}"
echo "Radio      : ${RADIO_NAME}"
echo "Features   : ${FEATURES}"
echo

# Build the Android-like userspace first so the PC-block image always matches
# the bootloader being flashed. Set ZEPHYR_SKIP_FIRMWARE_BUILD=1 only when
# intentionally re-flashing an unchanged image.
if [[ "$STORAGE_NAME" == "pc" && "${ZEPHYR_SKIP_FIRMWARE_BUILD:-0}" != "1" ]]; then
  if [[ -x "$FIRMWARE_DIR/build.sh" ]]; then
    echo "== Building Firmware =="
    (cd "$FIRMWARE_DIR" && ./build.sh)
  else
    echo "Firmware build script not found: $FIRMWARE_DIR/build.sh" >&2
    exit 1
  fi
fi

DEFAULT_FIRMWARE_IMAGE="$FIRMWARE_DIR/out/target/product/zero/zephyr-watch-sd.img"
IMAGE="${ZEPHYR_PC_IMAGE:-$DEFAULT_FIRMWARE_IMAGE}"

if [[ "$STORAGE_NAME" == "pc" && ! -f "$IMAGE" ]]; then
  echo "Firmware image was not produced: $IMAGE" >&2
  exit 1
fi

if [[ "$STORAGE_NAME" == "pc" ]] && ! python3 -c 'import serial' >/dev/null 2>&1; then
  cat >&2 <<'MSG'
pyserial is required for PC-block mode.
On Arch Linux:
  sudo pacman -S python-pyserial
MSG
  exit 1
fi

echo
echo "== Building Bootloader =="
cargo +esp build --release "${FLAGS[@]}"

echo
echo "== Flashing ESP32-S3 =="
if [[ "$STORAGE_NAME" == "pc" ]]; then
  cargo espflash flash --release --chip esp32s3 "${FLAGS[@]}"
else
  cargo espflash flash --release --monitor --chip esp32s3 "${FLAGS[@]}"
  exit 0
fi

# PC mode: the block server must be the only owner of ttyACM/ttyUSB.  Start it
# immediately after espflash releases the port; the firmware waits up to ~30 s
# for the first disk reply, so this avoids the old two-terminal race.
echo
echo "== Starting PC block server =="
python3 -u usb_block_server.py --image "$IMAGE" &
SERVER_PID=$!

cleanup() {
  if kill -0 "$SERVER_PID" 2>/dev/null; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

SOCKET="${XDG_RUNTIME_DIR:-/tmp}/zephyr-watch-viewer.sock"
if [[ -z "${XDG_RUNTIME_DIR:-}" ]]; then
  SOCKET="/tmp/zephyr-watch-viewer-$(id -u).sock"
fi

for _ in $(seq 1 80); do
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "usb_block_server.py exited unexpectedly" >&2
    wait "$SERVER_PID" || true
    exit 1
  fi
  [[ -S "$SOCKET" ]] && break
  sleep 0.1
done

# Bring up the live watch viewer automatically when a GUI session is present.
if [[ -n "${WAYLAND_DISPLAY:-}${DISPLAY:-}" ]] && python3 -c 'import tkinter' >/dev/null 2>&1; then
  echo
echo "== Starting live viewer =="
  python3 desktop_viewer.py || true
  echo
  echo "Viewer closed. PC block server is still running; Ctrl+C stops the session."
else
  echo
echo "GUI viewer not started (no GUI session or tkinter missing)."
  echo "PC block server is running; Ctrl+C stops the session."
fi

wait "$SERVER_PID"
