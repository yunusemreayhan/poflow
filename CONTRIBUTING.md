# Contributing

## Prerequisites

- Rust 1.94+ (`rustup update stable`)
- Node.js 22+ and npm
- SQLite 3.35+ (for WAL mode and RETURNING)
- Python 3.12+ (for E2E tests)

## Setup

```bash
# Backend
cargo build -p poflow-daemon

# Frontend
cd gui && npm ci

# E2E tests
cd e2etests && python -m venv .venv && source .venv/bin/activate && pip install -r requirements.txt
```

## Running Tests

```bash
# ALL quality gates at once (recommended)
./check.sh

# Or individually:
cargo test -p poflow-daemon          # 416+ backend tests
cd gui && npm test                      # 154+ frontend tests
cargo clippy -p poflow-daemon -- -D warnings  # zero warnings required
```

## Quality Gates — ALL MUST PASS

Every PR and commit must pass these gates. Run `./check.sh` to verify all at once.

1. **Backend tests** — `cargo test -p poflow-daemon` — zero failures
2. **Frontend tests** — `cd gui && npm test` — zero failures
3. **Clippy** — `cargo clippy -p poflow-daemon -- -D warnings` — zero warnings
4. No test disabling — fix code or adapt test, never skip
5. New features MUST have tests

## Development Workflow

1. Make changes
2. Run `./check.sh` (all 4 gates: frontend tests, backend tests, clippy, frontend build)
3. Commit with descriptive message: `feat:`, `fix:`, `test:`, `perf:`, `docs:`, `security:`
4. Every new feature MUST have tests

## Code Style

- Backend: `cargo fmt` + `cargo clippy -- -D warnings`
- Frontend: follow existing patterns, TypeScript strict mode
- Keep functions short, extract helpers for repeated patterns
- Add `#[utoipa::path]` annotations to all new endpoints
- Add new endpoints to the `#[openapi]` macro in `main.rs`

## Architecture

- Backend: axum + SQLite WAL + JWT auth + SSE + WebSocket
- Frontend: React 19 + Zustand + Tailwind v4 + Vite
- CLI: clap + reqwest (30 subcommands)
- 165 API routes, 28 database indexes, 20 migrations

## Deployment

```bash
# Docker (recommended)
docker compose up -d

# From source
./install.sh

# .deb package
cargo deb -p poflow-daemon
```

## Key Files

| Area | Path |
|---|---|
| Router + middleware | `crates/poflow-daemon/src/lib.rs` |
| Auth (JWT, CSRF) | `crates/poflow-daemon/src/auth.rs` |
| Route handlers | `crates/poflow-daemon/src/routes/*.rs` |
| DB layer | `crates/poflow-daemon/src/db/*.rs` |
| Frontend app | `gui/src/App.tsx` |
| Zustand store | `gui/src/store/` |
| Platform abstraction | `gui/src/platform.ts` |
| Backend tests | `crates/poflow-daemon/tests/api_tests.rs` |
| E2E tests | `e2etests/` |
| Quality gate script | `check.sh` |
