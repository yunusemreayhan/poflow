# Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Tauri v2 Desktop App                     │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                    React + TypeScript GUI                  │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌─────────────┐  │  │
│  │  │ TaskList  │ │  Timer   │ │  Rooms   │ │  Sprints    │  │  │
│  │  │ TaskNode  │ │          │ │ RoomView │ │  Board      │  │  │
│  │  │ Detail    │ │          │ │          │ │  Burndown   │  │  │
│  │  └────┬─────┘ └────┬─────┘ └────┬─────┘ └──────┬──────┘  │  │
│  │       │             │            │              │          │  │
│  │  ┌────▼─────────────▼────────────▼──────────────▼──────┐  │  │
│  │  │              Zustand Store + API Layer               │  │  │
│  │  │   store.ts (state)  │  api.ts (HTTP + SSE client)   │  │  │
│  │  └─────────────────────┼───────────────────────────────┘  │  │
│  └────────────────────────┼──────────────────────────────────┘  │
│                           │ HTTP / SSE / WebSocket               │
│  ┌────────────────────────┼──────────────────────────────────┐  │
│  │              Tauri Bridge (src-tauri/lib.rs)               │  │
│  │         write_file (safe dirs only) + shell open           │  │
│  └────────────────────────┼──────────────────────────────────┘  │
└───────────────────────────┼─────────────────────────────────────┘
                            │
                   HTTP :9090 (localhost)
                            │
┌───────────────────────────▼─────────────────────────────────────┐
│                    pomodoro-daemon (Rust/axum)                   │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │   auth.rs    │  │  engine.rs   │  │   routes/ (22+ mods)   │ │
│  │  JWT + HMAC  │  │  Per-user    │  │  auth, tasks, rooms,   │ │
│  │  Blocklist   │  │  timer FSM   │  │  sprints, burns, teams │ │
│  │  (persisted) │  │  tick loop   │  │  epics, labels, deps,  │ │
│  │  Rate limit  │  │  auto-start  │  │  webhooks, audit,      │ │
│  └──────────────┘  │  notify pref │  │  recurrence, export    │ │
│  ┌──────────────┐  └──────┬───────┘  └───────────┬────────────┘ │
│  │ webhook.rs   │         │                      │               │
│  │ HTTP dispatch│         │                      │               │
│  └──────────────┘         │                      │               │
│  ┌────────────────────────▼──────────────────────▼────────────┐ │
│  │                db/ (16 submodules + types.rs)               │ │
│  │  mod.rs: schema, connect, migrate (28 tables)              │ │
│  │  users, tasks, sessions, comments, assignees, rooms,       │ │
│  │  sprints, burns, epics, teams, audit, labels, recurrence,  │ │
│  │  dependencies, webhooks                                    │ │
│  └────────────────────────┬───────────────────────────────────┘ │
│                           │                                      │
│  ┌────────────────────────▼───────────────────────────────────┐ │
│  │              SQLite (WAL mode, pool=2)                      │ │
│  │         ~/.local/share/pomodoro/pomodoro.db                 │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │  notify.rs   │  │  config.rs   │  │  Swagger UI            │ │
│  │  libnotify   │  │  TOML file   │  │  /swagger-ui/          │ │
│  └──────────────┘  └──────────────┘  └────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│                    pomodoro-cli (Rust/reqwest)                    │
│  HTTP client → same daemon API                                   │
│  Env: POMODORO_URL, POMODORO_TOKEN                               │
└──────────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **Timer tick**: `main.rs` runs a 1s `tokio::interval` → `engine.tick()` advances all running timers → completed sessions trigger DB writes (outside lock) → `watch::Sender` broadcasts state → SSE streams push to connected clients

2. **Task CRUD**: GUI → `apiCall()` → HTTP `POST /api/tasks` → `routes/tasks.rs` validates → `db/tasks.rs` writes → `engine.notify(Tasks)` → `broadcast::Sender` → SSE `change` event → GUI reloads

3. **Estimation rooms**: GUI → HTTP or WebSocket → `routes/rooms.rs` → `db/rooms.rs` → `engine.notify(Rooms)` → WS/SSE push → all room members see updates

4. **Auth flow**: Register/Login → bcrypt (spawn_blocking) → JWT issued (7d expiry) → stored in Tauri Rust backend (filesystem) → `Authorization: Bearer` header → `Claims` extractor validates + checks blocklist (persisted in SQLite) → Logout revokes token

5. **Webhook dispatch**: Task CRUD → `webhook::dispatch()` spawns async task → queries `get_active_webhooks()` → HTTP POST to each URL with event payload

6. **Recurring tasks**: Background job (5min interval) → `get_due_recurrences()` → clones template task → `advance_recurrence()` → notifies SSE

## Key Design Decisions

- **Per-user timer state**: `HashMap<user_id, EngineState>` behind `Arc<Mutex<>>` — each user has independent timer
- **Optimistic locking**: `expected_updated_at` field on task/sprint updates → 409 Conflict on stale writes
- **SSE tickets**: Short-lived opaque tokens (30s) exchanged via `POST /api/timer/ticket` to avoid JWT in URL query strings
- **Structured errors**: All error responses return `{"error":"...","code":"..."}` via `ApiError` type
- **Module split**: `db/` (16 files + types.rs) and `routes/` (22+ files + types.rs) keep each domain under ~170 lines
- **Token blocklist**: Persisted to SQLite `token_blocklist` table, loaded into memory on startup
- **Webhook dispatch**: Fire-and-forget via `tokio::spawn`, 10s timeout per hook, SSRF protection on URL validation
- **Request body limit**: 2MB max via `DefaultBodyLimit` layer
