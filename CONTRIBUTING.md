# Contributing to Pomodoro Linux

## Development Setup

### Prerequisites
- Rust (stable, latest)
- Node.js 20+
- SQLite 3
- Linux with libnotify, libwebkit2gtk-4.1, libsoup-3.0, libgtk-3

### Quick Start

```bash
# Backend only (daemon + CLI)
cargo build -p pomodoro-daemon -p pomodoro-cli
cargo test -p pomodoro-daemon

# Frontend
cd gui && npm ci && npm run dev

# Full Tauri app
cd gui && cargo tauri dev
```

### Project Structure

```
crates/
  pomodoro-daemon/     # Rust HTTP backend (axum + SQLite)
    src/
      db/              # Database layer (10 submodules)
      routes/          # HTTP route handlers (16 submodules)
      engine.rs        # Per-user timer state machine
      auth.rs          # JWT authentication
      config.rs        # TOML configuration
      notify.rs        # Desktop notifications
  pomodoro-cli/        # CLI client (reqwest)
gui/
  src/
    components/        # React components
    store/             # Zustand state + API types
    __tests__/         # Vitest unit tests
  src-tauri/           # Tauri v2 bridge
```

### Running Tests

```bash
# Backend (69 integration tests, in-memory SQLite)
cargo test -p pomodoro-daemon

# Frontend (15 unit tests)
cd gui && npm test

# TypeScript type check
cd gui && npx tsc --noEmit
```

### Code Style

- Rust: `cargo fmt` + `cargo clippy`
- TypeScript: `npm run lint`
- Keep route handlers thin — business logic goes in `db/` modules
- All timestamps use `now_str()` helper (ISO 8601 with milliseconds)
- Status values are validated against constants in `routes/mod.rs`

### Database

- SQLite with WAL mode, 2 connections max
- Schema defined in `db/mod.rs::migrate()`
- Visual schema: `docs/schema.dbml` (paste into dbdiagram.io)
- All user references use `user_id` FK — usernames are changeable

### Adding a New Endpoint

1. Add the DB function in the appropriate `db/*.rs` submodule
2. Add the route handler in the appropriate `routes/*.rs` submodule
3. Register the route in `lib.rs::build_router()`
4. Add `#[utoipa::path]` annotation for Swagger
5. Add integration test in `tests/api_tests.rs`

### Building Packages

```bash
cd gui && cargo tauri build --bundles deb
# Output: target/release/bundle/deb/Pomodoro_*.deb
```

## Project Structure

```
crates/
  pomodoro-daemon/     # Rust HTTP backend (axum + SQLite)
    src/db/            # Database layer (one file per entity)
    src/routes/        # HTTP handlers (one file per resource)
    src/engine.rs      # Timer engine (per-user state machine)
    src/auth.rs        # JWT auth + token blocklist
  pomodoro-cli/        # CLI client
gui/
  src/components/      # React components
  src/store/           # Zustand store + API types
  src/i18n.ts          # Internationalization
  src-tauri/           # Tauri backend (Rust)
```

## Frontend Patterns

- **Zustand** store — single store, optimistic updates for mutations
- **`useT()`** hook from `i18n.ts` for user-facing strings
- CSS: Tailwind utility classes + CSS variables for theming

### Adding a New Locale
1. Copy the `en` object in `gui/src/i18n.ts`
2. Translate all values
3. Add to the `locales` map

## Backend Patterns

### Adding a New Endpoint
1. DB function in `src/db/<entity>.rs`
2. Route handler in `src/routes/<entity>.rs`
3. Register in `routes/mod.rs` + `lib.rs`
4. Add `#[utoipa::path]` + register in `ApiDoc` (main.rs)
