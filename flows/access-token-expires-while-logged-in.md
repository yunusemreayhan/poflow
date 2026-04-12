# Flow: Access Token Expires While User Is Logged In

## Scenario
User logged in 2+ hours ago. Access token has expired. User performs an action.

## Steps

### Backend Side
1. User's request arrives with expired access token.
2. `Claims::from_request_parts` → `verify_token()` fails (expired) → `401 Unauthorized`.

### GUI Side (api.ts)
1. `apiCall` catches the error, detects `401`/`Unauthorized` in message.
2. Calls `tryRefreshToken()`:
   - Reads `savedServers[0].refresh_token` from store.
   - Sends `POST /api/auth/refresh` with `{"refresh_token": "..."}`.
3. **If refresh succeeds** (refresh token still valid, ≤30 days):
   - New access token + new refresh token returned.
   - Old refresh token revoked (rotation).
   - Token updated in Tauri backend, savedServers, and store.
   - Original request retried with new token → succeeds.
   - **User never sees an error.**
4. **If refresh fails** (refresh token also expired/revoked/invalid):
   - `tryRefreshToken()` returns `false`.
   - **Auto-logout triggered** (our recent fix).
   - User sees login screen.

### SSE Connection
- SSE uses a separate ticket-based auth (not JWT directly).
- When access token expires, the SSE connection itself stays alive (ticket was already consumed).
- But if SSE disconnects and tries to reconnect, it needs a new ticket → needs valid access token → fails → `connected: false` → red indicator.
- The SSE reconnect triggers `apiCall("POST", "/api/timer/ticket")` → 401 → refresh flow kicks in.

## Token Refresh Details (Backend)
1. `POST /api/auth/refresh` with `{"refresh_token": "..."}`.
2. Rate limited (10/min per IP).
3. Check if refresh token is revoked → `401`.
4. Verify JWT signature and expiry → `401`.
5. Verify `claims.typ == "refresh"` → `401`.
6. **Re-fetch user from DB** — gets current `username` and `role` (not stale claims).
7. Issue new access token + new refresh token.
8. **Revoke old refresh token** (one-time use / rotation).
9. Return new tokens.

## ⚠️ Note: Refresh Token Rotation
Each refresh creates a new refresh token and revokes the old one. This means:
- If two tabs/devices share the same refresh token, the first to refresh invalidates the other.
- The GUI stores refresh tokens per-server in `savedServers`, but only `savedServers[0]` is used for auto-refresh.
