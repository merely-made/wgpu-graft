#!/usr/bin/env bash
# Launch the already-built reference demo through macOS LaunchServices. A
# self-hosted runner worker can compile AppKit code but does not reliably drive
# a directly spawned GUI process; a minimal .app gives the smoke gate the
# logged-in GUI context it needs.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_ROOT="${CARGO_TARGET_DIR:-$REPO/target}"
BINARY="${1:-$TARGET_ROOT/debug/demo-servo-winit}"
RECEIPT="GRAFT DEMO SMOKE PASS"

if [[ ! -x "$BINARY" ]]; then
    echo "demo-servo-winit executable not found at $BINARY" >&2
    exit 1
fi

SCRATCH="$(mktemp -d "${RUNNER_TEMP:-/tmp}/graft-demo-mac.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT
APP="$SCRATCH/demo-servo-winit.app"
MACOS="$APP/Contents/MacOS"
PLIST="$APP/Contents/Info.plist"
STDOUT="$SCRATCH/demo.stdout.log"
STDERR="$SCRATCH/demo.stderr.log"

mkdir -p "$MACOS"
cp "$BINARY" "$MACOS/demo-servo-winit"
chmod 755 "$MACOS/demo-servo-winit"
plutil -create xml1 "$PLIST"
plutil -insert CFBundleExecutable -string demo-servo-winit "$PLIST"
plutil -insert CFBundleIdentifier -string made.merely.wgpu-graft.demo-servo-winit "$PLIST"
plutil -insert CFBundleName -string demo-servo-winit "$PLIST"
plutil -insert CFBundlePackageType -string APPL "$PLIST"
plutil -insert NSHighResolutionCapable -bool true "$PLIST"

set +e
open -W -n -F --stdout "$STDOUT" --stderr "$STDERR" \
    --env "WGPU_BACKEND=${WGPU_BACKEND:-metal}" \
    "$APP" --args --smoke
code=$?
set -e

[[ ! -s "$STDOUT" ]] || sed -n '1,240p' "$STDOUT"
[[ ! -s "$STDERR" ]] || sed -n '1,240p' "$STDERR" >&2

if [[ $code -ne 0 ]]; then
    exit "$code"
fi
if ! grep -Fq "$RECEIPT" "$STDOUT" "$STDERR"; then
    echo "demo exited without required receipt: $RECEIPT" >&2
    exit 1
fi
echo "Deterministic macOS smoke passed."
