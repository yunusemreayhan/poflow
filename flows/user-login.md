# Flow: User Login (Initial)

## Actor
Unauthenticated user with existing account.

## Steps

1. User sends `POST /api/auth/login` with `{"username": "alice", "password": "MyPass123"}`.
2. **Rate limiting**: 10 requests per 60 seconds per IP.
3. Lookup user by username → `401 "Invalid credentials"` if not found.
4. Verify password with bcrypt → `401 "Invalid credentials"` if wrong.
5. **Bcrypt cost upgrade**: if stored hash uses cost < 12, rehash with cost 12 and update DB (transparent to user).
6. Create access token (2h default).
7. Create refresh token (30 days default).
8. Return `200 OK` with `{token, refresh_token, user_id, username, role}`.

## GUI Behavior
1. AuthScreen sends login request.
2. On success:
   - `setToken(resp.token)` → sets token in Tauri HTTP client state.
   - `invoke("save_auth", ...)` → encrypts and saves to `~/.local/share/poflow-gui/.auth` (AES-256-GCM, key derived from hostname+username+salt).
   - Saves to `savedServers` list in localStorage.
   - Sets `{token, username, role}` in Zustand store.
3. `useSseConnection` hook fires (token changed) → establishes SSE connection.
4. `loadTasks()` called → fetches all tasks.
5. Backend indicator turns green.

## Token Structure (JWT Claims)
```json
{
  "sub": "1",
  "user_id": 1,
  "username": "alice",
  "role": "user",
  "exp": 1775984003,
  "iat": 1775976803,
  "typ": "access"
}
```

## Security
- CSRF protection: `X-Requested-With` header required on all non-GET requests.
- Tokens are HS256-signed with the server's JWT secret.
- No same-origin check — any client with the token can use the API.
