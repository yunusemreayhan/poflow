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
