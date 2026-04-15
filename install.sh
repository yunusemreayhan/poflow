#!/bin/bash
# install.sh — Install pomodoroLinux from source
set -e

echo "Building pomodoro-daemon..."
cargo build --release -p pomodoro-daemon

echo "Building web GUI..."
cd gui && npm ci && npm run build && cd ..

echo "Installing binary..."
sudo install -Dm755 target/release/pomodoro-daemon /usr/bin/pomodoro-daemon

echo "Installing web GUI..."
sudo mkdir -p /usr/share/pomodoro/gui
sudo cp -r gui/dist/* /usr/share/pomodoro/gui/

echo "Installing systemd service..."
sudo install -Dm644 assets/pomodoro.service /usr/lib/systemd/user/pomodoro.service

echo "Installing desktop entry..."
sudo install -Dm644 assets/pomodoro.desktop /usr/share/applications/pomodoro.desktop

echo ""
echo "Installation complete! Start with:"
echo "  systemctl --user daemon-reload"
echo "  systemctl --user enable --now pomodoro"
echo ""
echo "Then open http://localhost:9090 in your browser."
