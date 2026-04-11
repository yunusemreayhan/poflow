# Deployment Guide

## Quick Start (Development)

```bash
# Start the backend
cd crates/pomodoro-daemon
cargo run

# Start the GUI (separate terminal)
cd gui
npm install
npm run tauri dev
```

The backend runs on `http://127.0.0.1:9090` by default.
Swagger UI: `http://127.0.0.1:9090/swagger-ui/`

## Production Deployment

### Backend as systemd Service

```ini
# /etc/systemd/system/pomodoro.service
[Unit]
Description=Pomodoro Timer Daemon
After=network.target

[Service]
Type=simple
User=pomodoro
Environment=POMODORO_JWT_SECRET=<your-secret-here>
Environment=RUST_LOG=pomodoro_daemon=info
ExecStart=/usr/local/bin/pomodoro-daemon
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable pomodoro
sudo systemctl start pomodoro
```

### Configuration

Config file: `~/.config/pomodoro/config.toml`

```toml
bind_address = "127.0.0.1"  # Use 0.0.0.0 for network access
bind_port = 9090
work_duration_min = 25
short_break_min = 5
long_break_min = 15
long_break_interval = 4
daily_goal = 8
```

### Reverse Proxy (nginx)

```nginx
server {
    listen 443 ssl;
    server_name pomodoro.example.com;

    location / {
        proxy_pass http://127.0.0.1:9090;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

### Database

SQLite database: `~/.local/share/pomodoro/pomodoro.db`
Attachments: `~/.local/share/pomodoro/attachments/`
JWT secret: `~/.local/share/pomodoro/.jwt_secret`

### Backup

```bash
# Backup database and attachments
cp ~/.local/share/pomodoro/pomodoro.db ~/backups/pomodoro-$(date +%F).db
cp -r ~/.local/share/pomodoro/attachments ~/backups/attachments-$(date +%F)
```

### Environment Variables

| Variable | Description | Default |
|---|---|---|
| `POMODORO_JWT_SECRET` | JWT signing secret | Auto-generated |
| `RUST_LOG` | Log level | `pomodoro_daemon=info` |

### Building for Production

```bash
# Backend
cargo build --release -p pomodoro-daemon
cargo build --release -p pomodoro-cli

# GUI (.deb package)
cd gui
npm run tauri build
# Output: gui/src-tauri/target/release/bundle/deb/
```

## SSE Events

The server sends real-time updates via Server-Sent Events at `GET /api/timer/sse?ticket=<ticket>`.

### Event Types

| Event | Trigger | Frontend Handler |
|---|---|---|
| `timer` | Timer state change (tick, start, stop, pause, resume, skip) | Updates engine state |
| `change` | Data mutation | Dispatches custom events based on `ChangeEvent` type |

### Change Event Types

| Type | Triggers On | Frontend Event |
|---|---|---|
| `Tasks` | Task CRUD, assignee add/remove, comment add/delete, time report | `sse-tasks` → reloads task list |
| `Sprints` | Sprint CRUD, start/complete, task add/remove, burn log/cancel | `sse-sprints` → reloads sprint data |
| `Rooms` | Room CRUD, join/leave, start voting, vote, reveal, accept | `sse-rooms` → reloads room state |

### Obtaining a Ticket

```
POST /api/timer/ticket
Authorization: Bearer <access_token>
→ { "ticket": "<one-time-use-ticket>" }
```

Tickets expire after 30 seconds and are single-use.

## Authorization Model

### Authentication
All API endpoints (except `/api/auth/register` and `/api/auth/login`) require a valid JWT access token in the `Authorization: Bearer <token>` header.

### Roles
- `root` — Full access to all resources. Can manage users, access admin endpoints.
- `user` — Standard access. Can only modify own resources.

### Endpoint Authorization

| Endpoint | Authorization |
|---|---|
| Tasks CRUD | Owner or root |
| Task reorder | Owner of first task, or root |
| Time reports | Task owner, assignee, or root |
| Sprints CRUD | Creator or root |
| Sprint tasks add/remove | Sprint creator or root |
| Sprint start/complete | Sprint creator or root |
| Burns log/cancel | Any authenticated user (team collaboration) |
| Rooms CRUD | Creator or root (for close/delete) |
| Room join/leave/vote | Any authenticated user |
| Assignees add/remove | Any authenticated user (team collaboration) |
| Comments add | Any authenticated user |
| Comments delete | Comment author or root |
| Webhooks | Own webhooks only |
| Config update | Root only |
| Admin endpoints | Root only |
| User list | Any authenticated user |
| Profile update | Own profile only |

## Rate Limiting

Auth endpoints (`/api/auth/login`, `/api/auth/register`, `/api/auth/refresh`) are rate-limited to **10 requests per 60 seconds per IP address**.

IP is extracted from `X-Forwarded-For` or `X-Real-IP` headers. IPv4-mapped IPv6 addresses (e.g., `::ffff:127.0.0.1`) are normalized to IPv4. If no IP header is present, the key `"unknown"` is used (all headerless requests share one bucket).

The general API has a separate rate limiter on the `ConnectInfo` socket address when running behind `axum::serve` with `into_make_service_with_connect_info`.

## Webhook Payload Format

Webhooks receive POST requests with JSON body:

```json
{
  "event": "task.created",
  "data": { "id": 42, "title": "My Task" }
}
```

**Event types:** `task.created`, `task.updated`, `task.deleted`, `sprint.created`, `sprint.started`, `sprint.completed`

**Headers:**
- `Content-Type: application/json`
- `X-Pomodoro-Event: <event_type>`
- `X-Pomodoro-Signature: sha256=<hex>` (if webhook has a secret — HMAC-SHA256)

**Retry policy:** 3 attempts with exponential backoff (2s, 4s, 8s).

## CORS Configuration

Default allowed origins: `http://localhost:1420`, `http://127.0.0.1:1420`, `http://localhost:9090`, `http://127.0.0.1:9090`, `tauri://localhost`.

Add custom origins via environment variable:

```bash
POMODORO_CORS_ORIGINS="https://app.example.com,https://staging.example.com" pomodoro-daemon
```

## Production Deployment

### Reverse Proxy (nginx)

```nginx
server {
    listen 443 ssl;
    server_name pomodoro.example.com;

    location / {
        proxy_pass http://127.0.0.1:9090;
        proxy_set_header X-Forwarded-For $remote_addr;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header Host $host;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

### systemd Service

```ini
[Unit]
Description=Pomodoro Daemon
After=network.target

[Service]
Type=simple
User=pomodoro
ExecStart=/usr/local/bin/pomodoro-daemon
Environment=POMODORO_CORS_ORIGINS=https://pomodoro.example.com
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### Backup

The SQLite database is at `~/.local/share/pomodoro/pomodoro.db`. Back up this file regularly. Attachments are stored in `~/.local/share/pomodoro/attachments/`.

## API Documentation

The backend serves interactive API documentation via Swagger UI at `/swagger-ui/`. All endpoints are annotated with request/response schemas, authentication requirements, and parameter descriptions.

Access it at: `http://localhost:9090/swagger-ui/`

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `POMODORO_CORS_ORIGINS` | (none) | Comma-separated additional CORS origins |
| `POMODORO_DB_PATH` | `~/.local/share/pomodoro/pomodoro.db` | SQLite database path |
| `JWT_SECRET` | (auto-generated) | JWT signing secret (set in production) |
| `POMODORO_HOST` | `0.0.0.0` | Bind address |
| `POMODORO_PORT` | `9090` | Bind port |
