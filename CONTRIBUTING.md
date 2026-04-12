# Contributing to pomodoroLinux

## Prerequisites

- Rust 1.75+ (with cargo)
- Node.js 18+ (with npm)
- SQLite 3.35+ (bundled via sqlx)

## Project Structure

```
crates/pomodoro-daemon/   # Backend (Rust, axum, SQLite)
  src/
    auth.rs               # JWT auth, token blocklist
    config.rs             # Server config (TOML)
    engine.rs             # Timer engine (per-user state)
    db/                   # Database layer (sqlx, SQLite)
    routes/               # HTTP route handlers
  tests/api_tests.rs      # Integration tests (211 tests)

gui/                      # Frontend (React, TypeScript, Tauri v2)
  src/
    store/                # Zustand store + API client
    components/           # React components
    hooks/                # Custom hooks (SSE, debounce)
    locales/              # i18n translations (en, tr)
  src/__tests__/          # Frontend tests (154 tests)
```

## Development Setup

### Backend

```bash
# Run the daemon (auto-creates SQLite DB)
cargo run -p pomodoro-daemon

# Run tests
cargo test -p pomodoro-daemon

# Check compilation
cargo check -p pomodoro-daemon
```

The daemon starts on `http://127.0.0.1:9090` by default.
Swagger UI available at `http://127.0.0.1:9090/swagger-ui/`.

### Frontend

```bash
cd gui

# Install dependencies
npm install

# Type check
npx tsc --noEmit

# Run tests
npm test

# Dev server (Tauri)
npm run tauri dev
```

## Testing

Always verify before committing:

```bash
# Backend (should show 211+ passed)
cargo test -p pomodoro-daemon

# Frontend (should show 154+ passed, TS clean)
cd gui && npx tsc --noEmit && npm test
```

## Environment Variables

See `docs/ENV_VARS.md` for all supported environment variables.

Key ones:
- `POMODORO_JWT_SECRET` — JWT signing secret (auto-generated if not set)
- `POMODORO_ROOT_PASSWORD` — Initial root user password (default: "root")
- `POMODORO_CORS_ORIGINS` — Comma-separated allowed origins
- `POMODORO_LOG_JSON=1` — Enable JSON structured logging

## Code Style

- Rust: standard `rustfmt` formatting
- TypeScript: strict mode, no `any` types
- Minimal code — only what's needed to solve the problem
- All endpoints need `#[utoipa::path]` annotations for Swagger
