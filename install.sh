#!/bin/bash
# install.sh — Install pomodoroLinux from source
set -e

echo "Building pomodoro-daemon..."
cargo build --release -p pomodoro-daemon

echo "Building pomodoro-cli..."
cargo build --release -p pomodoro-cli

echo "Building web GUI..."
cd gui && npm ci && npm run build && cd ..

echo "Installing binaries..."
sudo install -Dm755 target/release/pomodoro-daemon /usr/bin/pomodoro-daemon
sudo install -Dm755 target/release/pomo /usr/bin/pomo

echo "Installing web GUI..."
sudo mkdir -p /usr/share/pomodoro/gui
sudo cp -r gui/dist/* /usr/share/pomodoro/gui/

echo "Installing systemd service..."
sudo install -Dm644 assets/pomodoro.service /usr/lib/systemd/user/pomodoro.service

echo "Installing desktop entry..."
sudo install -Dm644 assets/pomodoro.desktop /usr/share/applications/pomodoro.desktop

echo "Installing icons..."
for size in 32 64 128 256; do
  sudo install -Dm644 "assets/icons/pomodoro-${size}.png" "/usr/share/icons/hicolor/${size}x${size}/apps/pomodoro.png"
done
sudo install -Dm644 assets/icons/pomodoro.svg /usr/share/icons/hicolor/scalable/apps/pomodoro.svg
sudo gtk-update-icon-cache /usr/share/icons/hicolor 2>/dev/null || true

echo ""
echo "Installation complete! Start with:"
echo "  systemctl --user daemon-reload"
echo "  systemctl --user enable --now pomodoro"
echo ""
echo "Then open http://localhost:9090 in your browser."
