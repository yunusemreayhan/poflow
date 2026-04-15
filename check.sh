#!/bin/bash
# check.sh — Run all quality gates before committing
# Usage: ./check.sh [--quick]
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓ $1${NC}"; }
fail() { echo -e "${RED}✗ $1${NC}"; exit 1; }
info() { echo -e "${YELLOW}→ $1${NC}"; }

cd "$(dirname "$0")"

# Gate 1: Frontend tests
info "Gate 1: Frontend tests (vitest)"
cd gui && npm test 2>&1 | tail -3 && cd .. || { cd ..; fail "Frontend tests failed"; }
pass "Frontend tests"

# Gate 2: Backend tests
info "Gate 2: Backend tests (cargo test)"
if [ "$1" = "--quick" ]; then
    cargo test -p pomodoro-daemon --lib 2>&1 | tail -3 || fail "Backend lib tests failed"
else
    cargo test -p pomodoro-daemon 2>&1 | tail -3 || fail "Backend tests failed"
fi
pass "Backend tests"

# Gate 3: Clippy
info "Gate 3: Clippy (zero warnings)"
cargo clippy -p pomodoro-daemon -- -D warnings 2>&1 | tail -1 || fail "Clippy warnings found"
pass "Clippy clean"

# Gate 4: Frontend build
info "Gate 4: Frontend build (tsc + vite)"
cd gui && npm run build 2>&1 | tail -3 && cd .. || { cd ..; fail "Frontend build failed"; }
pass "Frontend builds"

echo ""
echo -e "${GREEN}All gates passed!${NC}"
