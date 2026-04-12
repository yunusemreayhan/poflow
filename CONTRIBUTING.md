# Contributing

## Prerequisites

- Rust 1.75+ (`rustup update stable`)
- Node.js 18+ and npm
- SQLite 3.35+ (for WAL mode and RETURNING)

## Setup

```bash
# Backend
cd crates/pomodoro-daemon
cargo build

# Frontend
cd gui
npm install
```

## Running Tests

```bash
# Backend (219+ integration tests)
cargo test -p pomodoro-daemon

# Frontend (154+ unit tests)
cd gui && npm test

# TypeScript strict check
cd gui && npx tsc --noEmit
```

## Development Workflow

1. Make changes
2. Run `cargo check -p pomodoro-daemon` (fast compilation check)
3. Run `cargo test -p pomodoro-daemon` (full test suite)
4. Run `cd gui && npx tsc --noEmit` (type check)
5. Run `cd gui && npm test` (frontend tests)
6. Commit with descriptive message

## Code Style

- Backend: standard Rust formatting (`cargo fmt`)
- Frontend: no explicit formatter — follow existing patterns
- Keep functions short, extract helpers for repeated patterns
- Add `#[utoipa::path]` annotations to all new endpoints
