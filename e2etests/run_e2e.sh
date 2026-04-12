#!/usr/bin/env bash
# Run the full E2E test suite for pomodoroLinux.
# Usage: ./run_e2e.sh [pytest args...]
# Example: ./run_e2e.sh -k TestLogin -v
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"
VENV="$SCRIPT_DIR/.venv"

# Check prerequisites
for cmd in cargo tauri-driver WebKitWebDriver; do
    command -v "$cmd" >/dev/null 2>&1 || { echo "ERROR: $cmd not found"; exit 1; }
done

# Ensure daemon binary is built
DAEMON="$REPO_DIR/target/release/pomodoro-daemon"
if [ ! -f "$DAEMON" ]; then
    echo "Building pomodoro-daemon (release)..."
    (cd "$REPO_DIR" && cargo build --release -p pomodoro-daemon)
fi

# Ensure GUI binary exists (check sibling repo or local)
GUI="${POMODORO_GUI_BINARY:-}"
if [ -z "$GUI" ]; then
    for candidate in \
        "$REPO_DIR/target/release/pomodoro-gui" \
        "$REPO_DIR/../pomodoroLinux/target/release/pomodoro-gui"; do
        if [ -f "$candidate" ]; then GUI="$candidate"; break; fi
    done
fi
if [ -z "$GUI" ] || [ ! -f "$GUI" ]; then
    echo "ERROR: pomodoro-gui binary not found. Build it or set POMODORO_GUI_BINARY."
    exit 1
fi

# Set up venv if needed
if [ ! -d "$VENV" ]; then
    echo "Creating venv..."
    python3 -m venv "$VENV"
    "$VENV/bin/pip" install -q pytest pytest-rerunfailures
    if [ -d "$SCRIPT_DIR/tauriTester" ]; then
        "$VENV/bin/pip" install -q -e "$SCRIPT_DIR/tauriTester"
    fi
fi

# Run tests
cd "$SCRIPT_DIR"
exec "$VENV/bin/python" -m pytest -v "$@"
