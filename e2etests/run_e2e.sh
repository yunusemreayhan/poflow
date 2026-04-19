#!/usr/bin/env bash
# Run the full E2E test suite for poflow.
# Each test file gets its own pytest invocation (fresh daemon + GUI session)
# to avoid state contamination between files.
#
# Automatically starts Xvfb on a free display so multiple instances
# can run in parallel on the same machine (CI-friendly).
#
# Usage: ./run_e2e.sh [pytest args...]
# Examples:
#   ./run_e2e.sh                    # run all test files
#   ./run_e2e.sh -k TestLogin -v    # pass-through to single pytest run
#   ./run_e2e.sh test_flows.py      # run specific file
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"
VENV="$SCRIPT_DIR/.venv"

# ── Check prerequisites ─────────────────────────────────────────
for cmd in cargo tauri-driver WebKitWebDriver; do
    command -v "$cmd" >/dev/null 2>&1 || { echo "ERROR: $cmd not found"; exit 1; }
done
command -v Xvfb >/dev/null 2>&1 || { echo "ERROR: Xvfb not found (apt install xvfb)"; exit 1; }

# ── Ensure binaries ─────────────────────────────────────────────
DAEMON="$REPO_DIR/target/release/poflow-daemon"
if [ ! -f "$DAEMON" ]; then
    echo "Building poflow-daemon (release)..."
    (cd "$REPO_DIR" && cargo build --release -p poflow-daemon)
fi

GUI="${POFLOW_GUI_BINARY:-}"
if [ -z "$GUI" ]; then
    for candidate in \
        "$REPO_DIR/target/release/poflow-gui" \
        "$REPO_DIR/../poflow/target/release/poflow-gui"; do
        if [ -f "$candidate" ]; then GUI="$candidate"; break; fi
    done
fi
if [ -z "$GUI" ] || [ ! -f "$GUI" ]; then
    echo "ERROR: poflow-gui binary not found. Build it or set POFLOW_GUI_BINARY."
    exit 1
fi

# ── Set up venv if needed ───────────────────────────────────────
if [ ! -d "$VENV" ]; then
    echo "Creating venv..."
    python3 -m venv "$VENV"
    "$VENV/bin/pip" install -q pytest pytest-rerunfailures
    if [ -d "$SCRIPT_DIR/tauriTester" ]; then
        "$VENV/bin/pip" install -q -e "$SCRIPT_DIR/tauriTester"
    fi
fi

# ── Start Xvfb on a free display ────────────────────────────────
XVFB_PID=""
cleanup() {
    [ -n "$XVFB_PID" ] && kill "$XVFB_PID" 2>/dev/null || true
    # Kill any leftover processes
    pkill -9 -f "tauri-driver|WebKitWebDriver|poflow-gui|poflow-daemon" 2>/dev/null || true
}
trap cleanup EXIT

for _ in $(seq 1 10); do
    DISPLAY_NUM=$((RANDOM % 500 + 99))
    if ! [ -e "/tmp/.X${DISPLAY_NUM}-lock" ]; then
        Xvfb ":${DISPLAY_NUM}" -screen 0 1280x720x24 &>/dev/null &
        XVFB_PID=$!
        sleep 0.5
        if kill -0 "$XVFB_PID" 2>/dev/null; then
            export DISPLAY=":${DISPLAY_NUM}"
            echo "Xvfb started on display $DISPLAY (pid $XVFB_PID)"
            break
        fi
        XVFB_PID=""
    fi
done

if [ -z "$XVFB_PID" ]; then
    echo "ERROR: Could not start Xvfb on any display"
    exit 1
fi

# ── Run tests ───────────────────────────────────────────────────
cd "$SCRIPT_DIR"

# If args given, pass through to a single pytest invocation
if [ $# -gt 0 ]; then
    "$VENV/bin/python" -m pytest -v "$@"
    exit $?
fi

# No args: run each test file separately (fresh daemon per file)
TOTAL_PASS=0
TOTAL_FAIL=0
TOTAL_SKIP=0
FAILED_FILES=()

for f in test_*.py; do
    echo ""
    echo "━━━ $f ━━━"
    # Kill leftovers from previous file
    pkill -9 -f "tauri-driver|WebKitWebDriver|poflow-gui|poflow-daemon" 2>/dev/null || true
    sleep 1

    if output=$("$VENV/bin/python" -m pytest "$f" -q --no-header --tb=line 2>&1); then
        status=0
    else
        status=$?
    fi
    echo "$output" | tail -1

    # Parse "X passed, Y failed, Z skipped" from last line
    last=$(echo "$output" | tail -1)
    p=$(echo "$last" | grep -oP '\d+ passed' | grep -oP '\d+' || echo 0)
    fl=$(echo "$last" | grep -oP '\d+ failed' | grep -oP '\d+' || echo 0)
    s=$(echo "$last" | grep -oP '\d+ skipped' | grep -oP '\d+' || echo 0)
    e=$(echo "$last" | grep -oP '\d+ error' | grep -oP '\d+' || echo 0)

    TOTAL_PASS=$((TOTAL_PASS + p))
    TOTAL_FAIL=$((TOTAL_FAIL + fl + e))
    TOTAL_SKIP=$((TOTAL_SKIP + s))

    if [ "$fl" -gt 0 ] || [ "$e" -gt 0 ]; then
        FAILED_FILES+=("$f")
        # Show failure details
        echo "$output" | grep "FAILED\|ERROR" | head -5
    fi
done

echo ""
echo "════════════════════════════════════════════════════════════"
echo "TOTAL: $TOTAL_PASS passed, $TOTAL_FAIL failed, $TOTAL_SKIP skipped"
if [ ${#FAILED_FILES[@]} -gt 0 ]; then
    echo "FAILED FILES: ${FAILED_FILES[*]}"
    exit 1
else
    echo "ALL FILES PASSED ✓"
    exit 0
fi
