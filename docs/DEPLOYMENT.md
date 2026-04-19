# Deployment Guide

## Prerequisites
- Rust 1.75+ (for building the daemon)
- Node.js 18+ (for building the GUI)
- SQLite 3.35+ (bundled via sqlx)

## Build

### Backend
```bash
cargo build --release -p poflow-daemon
# Binary: target/release/poflow-daemon
```

### Frontend (Tauri desktop app)
```bash
cd gui
npm install
npm run tauri build
# Output: gui/src-tauri/target/release/bundle/
```

## Configuration

1. Copy default config: `mkdir -p ~/.config/poflow && cp config.example.toml ~/.config/poflow/config.toml`
2. Set environment variables (see [ENV_VARS.md](ENV_VARS.md))
3. Key settings:
   - `POFLOW_JWT_SECRET` — set a stable secret for production
   - `POFLOW_ROOT_PASSWORD` — change from default before first run
   - `POFLOW_CORS_ORIGINS` — restrict to your frontend origin

## Run

```bash
# Start daemon
./target/release/poflow-daemon

# Or with structured logging
POFLOW_LOG_JSON=1 ./target/release/poflow-daemon
```

The daemon creates its SQLite database at `~/.local/share/poflow/poflow.db` on first run.

## Systemd Service (Linux)

```ini
[Unit]
Description=Poflow Daemon
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/poflow-daemon
Environment=POFLOW_JWT_SECRET=your-secret-here
Environment=POFLOW_LOG_JSON=1
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

## Health Check

```bash
curl http://localhost:3030/api/health
# {"status":"ok","db":true,"active_timers":0}
```

## Backup

The SQLite database is a single file. Back it up with:
```bash
sqlite3 ~/.local/share/poflow/poflow.db ".backup /path/to/backup.db"
```

Attachments are stored in `~/.local/share/poflow/attachments/`.
