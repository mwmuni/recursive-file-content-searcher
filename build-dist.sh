#!/usr/bin/env bash
# build-dist.sh - produce installer, portable, and zip dist artifacts for Windows.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/electron-ui"

echo "[BUILD-DIST] Running clean"
npm run clean

echo "[BUILD-DIST] Building NSIS installer"
electron-builder --win nsis

echo "[BUILD-DIST] Building portable package"
electron-builder --win portable

echo "[BUILD-DIST] Building zip package"
electron-builder --win zip

echo "[BUILD-DIST] Completed: installer, portable, zip artifacts are in electron-ui/dist"