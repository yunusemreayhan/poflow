# Flow: User Registration

## Actor
Unauthenticated user (no token required).

## Steps

1. User sends `POST /api/auth/register` with `{"username": "alice", "password": "MyPass123"}`.
2. **Rate limiting**: 10 requests per 60 seconds per IP (`check_auth_rate_limit`).
3. **Username validation** (`validate_username`):
   - Non-empty, max 32 chars.
   - Alphanumeric + underscore + hyphen only.
4. **Password validation** (`validate_password`):
   - Min 8 characters, max 128.
   - Must contain at least one uppercase letter.
   - Must contain at least one digit.
5. Password hashed with bcrypt cost 12 (in blocking thread).
6. `db::create_user` inserts with role `"user"` (hardcoded).
   - If username exists → `409 Conflict "Username already taken"`.
7. Audit log entry: `action: "register"`.
8. Access token created (2h expiry, configurable via `ACCESS_TOKEN_EXPIRY_SECS`).
9. Refresh token created (30 day expiry, configurable via `REFRESH_TOKEN_EXPIRY_SECS`).
10. Returns `200 OK` with `{token, refresh_token, user_id, username, role}`.

## GUI Behavior
- AuthScreen shows Register form.
- On success: saves auth to Tauri secure store + localStorage, saves to `savedServers` list, sets token in Tauri backend.
- SSE connection starts automatically via `useSseConnection` hook.

## Authorization
- **No token required** — public endpoint.
- **Always creates role `"user"`** — cannot self-register as root.

## ⚠️ BUG: No Registration Disable Option

There is no way to disable public registration. Any person who can reach the daemon can create accounts. In a team environment, this may be undesirable.

See `backlog/no-registration-disable.md`.
