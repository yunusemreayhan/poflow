# Flow: User Relogs In After Long Absence (Both Tokens Expired)

## Scenario
User hasn't used the app for 30+ days. Both access token (2h) and refresh token (30d) have expired.

## Steps

### GUI Startup
1. App launches, `restoreAuth()` runs:
   - Tries `invoke("load_auth")` → decrypts `~/.local/share/poflow-gui/.auth`.
   - Falls back to `localStorage.getItem("auth")`.
   - If found: sets `{token, username, role}` in store.
2. `useSseConnection` fires (token is set):
   - Sends `POST /api/timer/ticket` with stale access token.
   - Backend returns `401` (token expired).
   - `apiCall` catches 401, calls `tryRefreshToken()`.
   - Refresh token also expired → `tryRefreshToken()` returns `false`.
   - **Auto-logout** (our fix) → clears token, shows login screen.
3. User sees login screen, enters credentials.
4. Fresh login → new access + refresh tokens → normal flow resumes.

### Without Our Fix (Old Behavior)
- `tryRefreshToken()` fails silently.
- User appears "logged in" (username shown, token in store).
- Every API call fails with 401.
- SSE never connects → red indicator.
- User stuck in broken limbo until they manually clear data.

## What Gets Cleared on Auto-Logout
- `token`, `username`, `role` set to `null` in Zustand store.
- `invoke("clear_auth")` → deletes `~/.local/share/poflow-gui/.auth`.
- `localStorage.removeItem("auth")`.
- `invoke("set_token", { token: "" })` → clears Tauri HTTP client token.
- `savedServers` list is **NOT** cleared — preserved for server switching.

## ⚠️ Note: Saved Server Tokens Are Stale
The `savedServers` list in localStorage retains the old tokens. When the user logs in again, the new tokens overwrite the entry for that server+username combo. But if the user switches to a different saved server, those tokens may also be stale.
