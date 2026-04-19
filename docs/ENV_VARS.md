# Environment Variables

All environment variables are optional. Defaults are shown.

| Variable | Default | Description |
|---|---|---|
| `POFLOW_JWT_SECRET` | auto-generated | JWT signing secret. If not set, a random 64-byte secret is generated and persisted to `~/.local/share/poflow/.jwt_secret`. |
| `POFLOW_CORS_ORIGINS` | localhost:1420,9090 | Comma-separated list of allowed CORS origins. Overrides config file `cors_origins`. |
| `POFLOW_LOG_JSON` | `false` | Set to `1` or `true` for JSON structured logging. |
| `POFLOW_SWAGGER` | `true` | Set to `0` or `false` to disable Swagger UI at `/swagger-ui/`. |
| `POFLOW_ROOT_PASSWORD` | `root` | Initial password for the auto-created `root` user. Only used on first run. |
| `ACCESS_TOKEN_EXPIRY_SECS` | `7200` (2h) | JWT access token lifetime in seconds. |
| `REFRESH_TOKEN_EXPIRY_SECS` | `2592000` (30d) | JWT refresh token lifetime in seconds. |
| `RUST_LOG` | `poflow_daemon=info` | Standard Rust log filter. Set to `debug` for verbose output. |
