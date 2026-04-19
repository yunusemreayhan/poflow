# Development Guide

## Prerequisites

- Rust 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- Node.js 18+ and npm
- SQLite 3.35+
- Linux: `libdbus-1-dev`, `libnotify-dev` (for desktop notifications)

## Quick Start

```bash
# Backend
cargo run -p poflow-daemon

# Frontend (dev server)
cd gui && npm install && npm run dev

# Tauri desktop app
cd gui && npm run tauri dev
```

## Build Commands

```bash
# Check compilation
cargo check -p poflow-daemon

# Run backend tests (168 tests)
cargo test -p poflow-daemon

# TypeScript check
cd gui && npx tsc --noEmit

# Frontend tests (132 tests)
cd gui && npm test

# Tauri bridge check
cd gui/src-tauri && cargo check

# Production build
cargo build --release -p poflow-daemon
cd gui && npm run tauri build
```

## Project Structure

```
crates/poflow-daemon/   # Rust backend (axum + SQLite)
  src/lib.rs              # Router, CORS, security headers
  src/main.rs             # Server startup, background tasks
  src/engine.rs           # Timer state machine
  src/auth.rs             # JWT auth, token blocklist
  src/config.rs           # Server configuration
  src/routes/             # API route handlers
  src/db/                 # Database queries (sqlx)
  src/webhook.rs          # Webhook dispatch
  src/notify.rs           # Desktop notifications
  tests/api_tests.rs      # Integration tests

gui/                      # React/TypeScript frontend
  src/App.tsx             # Main app, sidebar, shortcuts
  src/store/              # Zustand store, API client
  src/components/         # UI components
  src/i18n.ts             # Internationalization
  src-tauri/              # Tauri v2 bridge

crates/poflow-cli/      # CLI client
docs/                     # Documentation
```

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `POFLOW_JWT_SECRET` | random | JWT signing secret |
| `POFLOW_ROOT_PASSWORD` | `root` | Initial root user password |
| `POFLOW_CORS_ORIGINS` | localhost | Comma-separated CORS origins |
| `POFLOW_SWAGGER` | `true` | Set to `0` to disable Swagger UI |
| `DATABASE_URL` | `sqlite:poflow.db` | SQLite database path |

## API Documentation

Start the server and visit `http://localhost:9090/swagger-ui/` for interactive API docs.

## Lock Ordering

When acquiring multiple locks in `engine.rs`, always acquire `config` before `states` to prevent deadlocks.
