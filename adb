#!/usr/bin/env bash
# Модуль команд и пакетов ADB.
set -Eeuo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
if [[ "${1:-}" == logcat ]]; then
  shift
  exec python3 "$ROOT/logcat.py" "$@"
fi
exec python3 "$ROOT/zadb.py" "$@"
