# Flow: User Logout

## Actor
Authenticated user.

## Steps

### Backend
1. User sends `POST /api/auth/logout` with Bearer token.
2. Token extracted from `Authorization` header (no `Claims` extractor — manual extraction).
3. `auth::revoke_token(token)`:
   - SHA-256 hash of token added to in-memory blocklist.
   - Hash + expiry persisted to `token_blocklist` table.
   - Expired entries pruned from DB.
   - If in-memory set > 1000 entries, synced with DB (trim).
4. Returns `204 No Content`.

### GUI
1. `logout()` in store:
   - Fires `POST /api/auth/logout` (fire-and-forget, errors ignored).
   - `invoke("clear_auth")` → deletes encrypted auth file.
   - `localStorage.removeItem("auth")`.
   - Sets `{token: null, username: null, role: null}`.
   - `invoke("set_token", { token: "" })` → clears Tauri HTTP client.
2. `useSseConnection` hook: token becomes `null` → effect cleanup runs → SSE closed.
3. AuthScreen renders (no token → login form shown).

## What Is NOT Revoked
- The **refresh token** is not revoked on logout. Only the access token is.
- If an attacker captured the refresh token, they could still use it to get a new access token after the user logs out.

## ⚠️ BUG: Refresh Token Not Revoked on Logout

The logout endpoint only revokes the access token sent in the `Authorization` header. The refresh token (stored client-side) is not sent to the logout endpoint and is not revoked.

See `backlog/logout-doesnt-revoke-refresh.md`.
