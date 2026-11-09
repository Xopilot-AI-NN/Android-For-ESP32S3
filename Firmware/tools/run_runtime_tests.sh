#!/usr/bin/env sh
# Host TLS bindings need the host LLVM, not the Xtensa libclang installed by espup.
set -eu
FIRMWARE_ROOT="$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)"
TEST_IMAGE="${1:-$FIRMWARE_ROOT/out/google-services-build/aosp-wear-sd.img}"
LLVM_TOOL="${LLVM_CONFIG:-$(command -v llvm-config || true)}"
if [ -n "$LLVM_TOOL" ]; then
  LIBCLANG_PATH="$("$LLVM_TOOL" --libdir)"
  CLANG_PATH="$("$LLVM_TOOL" --bindir)/clang"
  export LIBCLANG_PATH CLANG_PATH
fi
HOST_TARGET="$(rustc +stable -vV | sed -n 's/^host: //p')"
export WATCH_TEST_IMAGE="$TEST_IMAGE"
export CARGO_ENCODED_RUSTFLAGS=''
exec cargo +stable test --locked --manifest-path "$FIRMWARE_ROOT/tools/runtime-check/Cargo.toml" --target "$HOST_TARGET" -- --include-ignored --test-threads=1
