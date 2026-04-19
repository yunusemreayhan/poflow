# Flow: GUI Auth Restore on App Startup

## Trigger
App launches, `useEffect` in `App` calls `restoreAuth()`.

## Steps

1. **Restore server URL**:
   - Read `serverUrl` from localStorage.
   - If found: set in store + `invoke("set_connection", { baseUrl })`.
   - Default: `http://127.0.0.1:9090`.

2. **Restore auth credentials**:
   - Try `invoke("load_auth")` → reads `~/.local/share/poflow-gui/.auth`, decrypts with AES-256-GCM.
   - Key derivation: `SHA-256(salt + hostname + username + "poflow-gui-auth-v2")`.
   - Salt: random 32 bytes, generated once, stored at `~/.local/share/poflow-gui/.auth_salt`.
   - Fallback: `localStorage.getItem("auth")` (unencrypted).
   - If found: parse JSON, set `{token, username, role}` in store, call `setToken(token)`.

3. **SSE connection starts** (via `useSseConnection` hook reacting to token change):
   - `POST /api/timer/ticket` → if token valid, SSE connects → green indicator.
   - If token expired → 401 → refresh flow → auto-logout if refresh also fails.

4. **Task loading**: `loadTasks()` called, fetches all data.

## Encryption Details
- Algorithm: AES-256-GCM.
- Key: 32 bytes from SHA-256 of `(salt : hostname : username : "poflow-gui-auth-v2")`.
- Nonce: random 12 bytes, prepended to ciphertext.
- Stored at: `~/.local/share/poflow-gui/.auth` (binary file).

## ⚠️ Note: Auth Tied to Machine Identity
The encryption key includes `hostname` and OS `username`. If either changes (e.g., hostname rename, different user account), the encrypted auth file becomes undecryptable. The fallback to localStorage will be used, but that's unencrypted.

## ⚠️ Note: No Token Validation on Restore
`restoreAuth` does not validate the token — it just sets it in the store. Validation happens on the first API call. This means the app briefly shows the "logged in" UI before potentially being kicked to the login screen.
