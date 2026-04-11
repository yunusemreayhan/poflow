# Operations Runbook

## Backup

```bash
# SQLite backup (while server is running)
sqlite3 pomodoro.db ".backup backup_$(date +%Y%m%d).db"

# Or copy the file (stop server first for consistency)
cp pomodoro.db pomodoro_backup_$(date +%Y%m%d).db
```

## Restore

```bash
# Stop the server, then:
cp backup_20260411.db pomodoro.db
# Restart the server
```

## User Management

```bash
# List users (via SQLite)
sqlite3 pomodoro.db "SELECT id, username, role, created_at FROM users;"

# Change user role
sqlite3 pomodoro.db "UPDATE users SET role = 'root' WHERE username = 'admin';"

# Reset user password (bcrypt hash for 'newpassword')
# Use the API instead: POST /api/auth/register (for new users)
```

## Log Analysis

```bash
# Run with debug logging
RUST_LOG=debug cargo run -p pomodoro-daemon

# Filter for errors only
RUST_LOG=error cargo run -p pomodoro-daemon

# Check for rate limiting hits
RUST_LOG=warn cargo run -p pomodoro-daemon 2>&1 | grep "Too many"
```

## Database Maintenance

```bash
# Check database integrity
sqlite3 pomodoro.db "PRAGMA integrity_check;"

# Vacuum (reclaim space after many deletes)
sqlite3 pomodoro.db "VACUUM;"

# Check database size
ls -lh pomodoro.db

# Prune expired token blocklist entries
sqlite3 pomodoro.db "DELETE FROM token_blocklist WHERE expires_at < datetime('now');"
```

## Configuration

Config file: `~/.config/pomodoro/config.toml`

```bash
# View current config
cat ~/.config/pomodoro/config.toml

# Reset to defaults (delete config, server recreates on start)
rm ~/.config/pomodoro/config.toml
```

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| 429 Too Many Requests | Rate limiter triggered | Wait 60s, or restart server to clear |
| JWT expired | Token older than 2h | Re-login or use refresh token |
| Database locked | Concurrent writes | Ensure single server instance |
| Notifications not showing | D-Bus not available | Install `libnotify` |
