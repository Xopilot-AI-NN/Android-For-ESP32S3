#!/usr/bin/env bash
# Модуль команд и пакетов ADB.
set -Eeuo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
if [[ "${1:-}" == push || "${1:-}" == -s || "${1:-}" == --serial ]]; then
  exec python3 "$ROOT/push.py" "$@"
fi
if [[ "${1:-}" == logcat ]]; then
  shift
  exec python3 "$ROOT/logcat.py" "$@"
fi
exec python3 "$ROOT/zadb.py" "$@"
