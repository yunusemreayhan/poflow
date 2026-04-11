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
