#!/bin/bash
# install.sh — Build and install pojidora from source (Ubuntu/Debian)
#
# Installs: pomodoro-daemon, pomo (CLI), web GUI, desktop GUI (optional)
#
# Usage:
#   ./install.sh              # daemon + CLI + web GUI (no desktop app)
#   ./install.sh --desktop    # also build and install Tauri desktop app
#   ./install.sh --deps-only  # only install system dependencies
#
# Requirements: Rust (rustup), Node.js 20+, npm
# For --desktop: cargo-tauri (`cargo install tauri-cli`)
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'; NC='\033[0m'
info() { echo -e "${YELLOW}→ $1${NC}"; }
pass() { echo -e "${GREEN}✓ $1${NC}"; }
fail() { echo -e "${RED}✗ $1${NC}"; exit 1; }

DESKTOP=false
DEPS_ONLY=false
for arg in "$@"; do
  case "$arg" in
    --desktop) DESKTOP=true ;;
    --deps-only) DEPS_ONLY=true ;;
  esac
done

cd "$(dirname "$0")"

# ── Step 1: System dependencies ─────────────────────────────────
info "Checking system dependencies..."

DEPS_BASE="build-essential pkg-config libssl-dev libsqlite3-dev"
DEPS_DESKTOP="libwebkit2gtk-4.1-dev libgtk-3-dev libappindicator3-dev librsvg2-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev"

MISSING=""
for pkg in $DEPS_BASE; do
  dpkg -s "$pkg" >/dev/null 2>&1 || MISSING="$MISSING $pkg"
done
if $DESKTOP; then
  for pkg in $DEPS_DESKTOP; do
    dpkg -s "$pkg" >/dev/null 2>&1 || MISSING="$MISSING $pkg"
  done
fi

if [ -n "$MISSING" ]; then
  info "Installing missing packages:$MISSING"
  sudo apt-get update -qq
  sudo apt-get install -y --no-install-recommends $MISSING
fi
pass "System dependencies"

if $DEPS_ONLY; then echo "Dependencies installed."; exit 0; fi

# ── Step 2: Check toolchain ─────────────────────────────────────
command -v cargo >/dev/null 2>&1 || fail "Rust not found. Install: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
command -v node >/dev/null 2>&1 || fail "Node.js not found. Install Node.js 20+ from https://nodejs.org"
command -v npm >/dev/null 2>&1 || fail "npm not found"
if $DESKTOP; then
  command -v cargo-tauri >/dev/null 2>&1 || fail "cargo-tauri not found. Install: cargo install tauri-cli"
fi
pass "Toolchain"

# ── Step 3: Build daemon + CLI ──────────────────────────────────
info "Building pomodoro-daemon (release)..."
cargo build --release -p pomodoro-daemon
pass "pomodoro-daemon"

info "Building pomodoro-cli (release)..."
cargo build --release -p pomodoro-cli
pass "pomodoro-cli (pomo)"

# ── Step 4: Build web GUI ───────────────────────────────────────
info "Building web GUI..."
cd gui && npm ci --prefer-offline && npx vite build && cd ..
pass "Web GUI (gui/dist)"

# ── Step 5: Build desktop GUI (optional) ────────────────────────
if $DESKTOP; then
  info "Building desktop GUI (Tauri — this takes a few minutes)..."
  # IMPORTANT: Must use 'cargo tauri build', NOT 'cargo build -p pomodoro-gui'.
  # 'cargo build' sets --cfg dev which makes the app load from localhost:1420
  # instead of the embedded frontend assets.
  cd gui && cargo tauri build --no-bundle 2>&1 | tail -3 && cd ..
  pass "Desktop GUI (pomodoro-gui)"
fi

# ── Step 6: Install ─────────────────────────────────────────────
info "Installing binaries..."
sudo install -Dm755 target/release/pomodoro-daemon /usr/bin/pomodoro-daemon
sudo install -Dm755 target/release/pomo /usr/bin/pomo

info "Installing web GUI..."
sudo mkdir -p /usr/share/pomodoro/gui
sudo cp -r gui/dist/* /usr/share/pomodoro/gui/

if $DESKTOP; then
  info "Installing desktop GUI..."
  sudo install -Dm755 target/release/pomodoro-gui /usr/bin/pomodoro-gui
fi

info "Installing systemd service..."
sudo install -Dm644 assets/pomodoro.service /usr/lib/systemd/user/pomodoro.service

info "Installing desktop entry and icons..."
sudo install -Dm644 assets/pomodoro.desktop /usr/share/applications/pomodoro.desktop
for size in 32 64 128 256; do
  if [ -f "assets/icons/pomodoro-${size}.png" ]; then
    sudo install -Dm644 "assets/icons/pomodoro-${size}.png" "/usr/share/icons/hicolor/${size}x${size}/apps/pomodoro.png"
  fi
done
if [ -f assets/icons/pomodoro.svg ]; then
  sudo install -Dm644 assets/icons/pomodoro.svg /usr/share/icons/hicolor/scalable/apps/pomodoro.svg
fi
sudo gtk-update-icon-cache /usr/share/icons/hicolor 2>/dev/null || true

pass "Installation complete"

# ── Step 7: Post-install ────────────────────────────────────────
echo ""
echo -e "${GREEN}╔══════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  pojidora installed successfully!        ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════╝${NC}"
echo ""
echo "Start the daemon:"
echo "  systemctl --user daemon-reload"
echo "  systemctl --user enable --now pomodoro"
echo ""
echo "Open web GUI:     http://localhost:9090"
if $DESKTOP; then
  echo "Open desktop GUI:  pomodoro-gui"
fi
echo "CLI:              pomo --help"
echo ""
echo "First user to register becomes admin."
