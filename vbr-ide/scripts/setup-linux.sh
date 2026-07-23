#!/usr/bin/env bash
#
# Install the system libraries the VBR IDE (Tauri v2) needs to build and run on
# a Debian/Ubuntu-based Linux. Ubuntu 24.04 ships WebKitGTK 4.1 (older releases
# used 4.0 — adjust the package name if you're on one of those).
#
# Run once:  ./scripts/setup-linux.sh
set -euo pipefail

echo "Installing Tauri's Linux build dependencies (needs sudo)…"
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libglib2.0-dev \
  libsoup-3.0-dev \
  librsvg2-dev \
  libayatana-appindicator3-dev \
  libxdo-dev \
  libssl-dev \
  build-essential \
  curl wget file pkg-config

echo
echo "Done. Next:"
echo "  cd vbr-ide"
echo "  npm install"
echo "  npm run tauri dev      # run it (needs a display; on WSL that's WSLg)"
echo "  npm run tauri build    # produce a .deb / .AppImage in src-tauri/target/release/bundle/"
