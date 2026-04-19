# Architecture

## System Overview

```
┌─────────────────────────────────────────────────┐
│  Frontend (Tauri v2 + React + TypeScript)        │
│  gui/src/                                        │
│  ├── App.tsx (sidebar, tabs, toasts, shortcuts)  │
│  ├── store/ (Zustand state + API client)         │
│  ├── components/ (Timer, TaskList, Sprints, etc) │
│  └── hooks/ (SSE, WebSocket, debounce)           │
└──────────────┬──────────────────┬────────────────┘
               │ REST/SSE         │ WebSocket
               ▼                  ▼
┌─────────────────────────────────────────────────┐
│  Backend (Rust + axum + SQLite)                  │
│  crates/poflow-daemon/src/                     │
│  ├── main.rs (server, background tasks)          │
│  ├── lib.rs (router, middleware, CORS)           │
│  ├── engine.rs (per-user timer state machine)    │
│  ├── auth.rs (JWT, token blocklist, CSRF)        │
│  ├── routes/ (HTTP handlers, 20+ modules)        │
│  ├── db/ (SQLite via sqlx, 19 modules)           │
│  ├── webhook.rs (outbound webhook dispatch)      │
│  └── notify.rs (desktop notifications)           │
└──────────────┬──────────────────────────────────┘
               │
               ▼
         SQLite (WAL mode)
         ~/.local/share/poflow/poflow.db
```

## Data Flow

1. Frontend calls `apiCall()` → Tauri `invoke("api_call")` → HTTP to backend
2. Backend processes request, updates SQLite, returns JSON
3. Backend broadcasts `ChangeEvent` via `watch`/`broadcast` channels
4. SSE stream picks up changes, sends to frontend
5. Frontend debounces and reloads affected data

## Auth Flow

1. `POST /api/auth/login` → bcrypt verify → JWT access + refresh tokens
2. Access token (2h) sent as `Authorization: Bearer <token>`
3. CSRF: mutation requests require `x-requested-with` header
4. Token refresh: `POST /api/auth/refresh` with refresh token (30d)
5. SSE/WebSocket: short-lived tickets via `POST /api/timer/ticket`

## Timer Engine

- Per-user state machine: Idle → Work → ShortBreak/LongBreak → Idle
- 1-second tick loop in background task
- Two-phase tick: advance timers (locked), then DB I/O (unlocked)
- Auto-start breaks/work based on user config
- Desktop notifications via `notify-rust`

## Key Design Decisions

- SQLite WAL mode for concurrent reads (4 connections)
- Soft delete for tasks (deleted_at column)
- Hierarchical tasks via parent_id with recursive CTEs
- ETag-based caching for `/api/tasks/full` endpoint
- Webhook secrets encrypted at rest (XOR + HMAC-derived key)

## Background Tasks

| Task | Interval | Purpose |
|---|---|---|
| Timer tick | 1s | Advance running timers, complete sessions, auto-start breaks |
| Sprint snapshot | 1h | Snapshot burndown data for active sprints and epic groups |
| Recurrence | 5min | Create recurring task instances when due |
| Auto-archive | 24h | Archive completed tasks older than 90 days |
| Attachment cleanup | 24h | Remove orphaned attachment files from disk |
| Due reminders | 30min | Desktop notifications for tasks due today/tomorrow |

All tasks report heartbeats to `/api/health` via `engine.heartbeat()`.
