#!/bin/bash
set -e

echo "Building Filesystem Searcher..."

echo
echo "Building Rust binary..."
cargo build --release

echo
echo "Building Electron UI..."
cd electron-ui

echo "Installing npm dependencies..."
npm install

echo "Building Electron app..."
npm run build

echo
echo "Build complete!"
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "The application can be found in: electron-ui/dist/mac/Filesystem Searcher.app"
else
    echo "The application can be found in: electron-ui/dist/linux-unpacked/filesystem-searcher"
fi
echo 