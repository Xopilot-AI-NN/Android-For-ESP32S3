#!/usr/bin/env sh
# One entry point for Linux/macOS and Android/Termux. Secrets are never sourced as shell code.
set -eu
GATEWAY_ROOT="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
CONFIG_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/aosp-wear-gateway/config.env"
ACTION="${1:-start}"
if [ "$#" -gt 0 ]; then shift; fi
case "$ACTION" in
  init)
    # Python imports the downloaded Web OAuth JSON into a mode-0600 file outside Git.
    exec python3 "$GATEWAY_ROOT/configure.py" "$@"
    ;;
  provision)
    exec python3 "$GATEWAY_ROOT/provision_watch.py" "$@"
    ;;
  test)
    node --test "$GATEWAY_ROOT"/tests/*.test.mjs
    exec python3 -m unittest discover -s "$GATEWAY_ROOT/tests" -p 'test_*.py'
    ;;
  help|-h|--help)
    echo 'Usage: bash run.sh init OAUTH_JSON [--base-url HTTPS_ORIGIN]'
    echo '       bash run.sh start | check | test'
    echo '       bash run.sh register [name]'
    echo '       bash run.sh providers PRIVATE_KEYS_JSON'
    echo '       bash run.sh ca'
    echo '       bash run.sh provision DEVICE_JSON CA_PEM DISK_IMAGE --base-url HTTPS_ORIGIN'
    echo '       bash run.sh pair DEVICE_JSON [calendar,gmail]'
    echo '       bash run.sh poll DEVICE_JSON PAIR_ID'
    echo '       bash run.sh confirm DEVICE_JSON PAIR_ID'
    echo '       bash run.sh renew DEVICE_JSON | logout SESSION_JSON'
    exit 0
    ;;
esac
command -v node >/dev/null 2>&1 || { echo 'Node.js missing. Termux: pkg install nodejs-lts python git' >&2; exit 1; }
node -e 'const [major,minor]=process.versions.node.split(".").map(Number);if(major<22||(major===22&&minor<13))throw Error("Node.js 22.13+ required");require("node:sqlite")'
[ -f "$CONFIG_FILE" ] || { echo 'Run init with your downloaded OAuth JSON first. Never copy the secret into this repository.' >&2; exit 1; }
case "$ACTION" in
  check)
    # Do not dump process.env or print OAuth values; verify only that config can be loaded.
    node --env-file="$CONFIG_FILE" "$GATEWAY_ROOT/src/check.mjs"
    ;;
  providers)
    exec node "$GATEWAY_ROOT/src/configure-providers.mjs" "$@"
    ;;
  ca)
    exec node --env-file="$CONFIG_FILE" "$GATEWAY_ROOT/src/export-ca.mjs"
    ;;
  start)
    # Android may otherwise suspend a long-lived terminal server when the screen sleeps.
    # Holding a wake lock does not disable Android's battery-management policy.
    if command -v termux-wake-lock >/dev/null 2>&1; then
      termux-wake-lock
      trap 'termux-wake-unlock >/dev/null 2>&1 || true' 0
    fi
    node --env-file="$CONFIG_FILE" "$GATEWAY_ROOT/src/server.mjs"
    ;;
  register|pair|poll|confirm|renew|logout)
    exec node --env-file="$CONFIG_FILE" "$GATEWAY_ROOT/src/admin.mjs" "$ACTION" "$@"
    ;;
  *) echo 'Unknown command; use help' >&2; exit 2 ;;
esac
