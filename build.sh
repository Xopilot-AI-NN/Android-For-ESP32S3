#!/usr/bin/env bash
# Модуль настройки сборки.
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
python3 "$ROOT/tools/build_firmware.py" "$@"
