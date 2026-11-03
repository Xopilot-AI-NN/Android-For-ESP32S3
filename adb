#!/usr/bin/env bash
# Модуль команд и пакетов ADB.
set -Eeuo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec "$ROOT/Bootloader/adb" "$@"
