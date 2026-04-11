# Environment Variables

| Variable | Default | Description |
|---|---|---|
| `POMODORO_JWT_SECRET` | Random 32-byte hex (generated at startup) | JWT signing secret. Set for stable tokens across restarts. |
| `POMODORO_ROOT_PASSWORD` | `root` | Initial root user password (only used on first DB init). |
| `POMODORO_CORS_ORIGINS` | `*` | Comma-separated allowed CORS origins (e.g. `http://localhost:1420,https://app.example.com`). |
| `POMODORO_SWAGGER` | `true` | Set to `0` or `false` to disable Swagger UI at `/swagger-ui/`. |
| `ACCESS_TOKEN_EXPIRY_SECS` | `7200` (2 hours) | JWT access token lifetime in seconds. |
| `REFRESH_TOKEN_EXPIRY_SECS` | `2592000` (30 days) | JWT refresh token lifetime in seconds. |

## Config File

Additional settings are in `~/.config/pomodoro/config.toml`:

| Key | Default | Description |
|---|---|---|
| `bind_address` | `127.0.0.1` | HTTP server bind address. |
| `bind_port` | `3030` | HTTP server bind port. |
| `work_duration_min` | `25` | Default work session duration (minutes). |
| `short_break_min` | `5` | Default short break duration (minutes). |
| `long_break_min` | `15` | Default long break duration (minutes). |
| `long_break_interval` | `4` | Work sessions before a long break. |
| `auto_start_breaks` | `true` | Auto-start break after work session. |
| `auto_start_work` | `false` | Auto-start work after break. |
| `daily_goal` | `8` | Daily completed sessions goal. |
