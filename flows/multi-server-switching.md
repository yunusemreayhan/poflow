# Flow: Multi-Server Switching

## Actor
User with accounts on multiple Poflow daemon instances.

## Data Model
`savedServers` in localStorage:
```json
[
  {"url": "http://127.0.0.1:9090", "username": "alice", "token": "...", "refresh_token": "...", "role": "root"},
  {"url": "http://192.168.1.50:9090", "username": "alice", "token": "...", "refresh_token": "...", "role": "user"}
]
```

## Steps

### Adding a Server
1. User changes server URL in AuthScreen → `setServerUrl(url)`.
2. Clears current auth state (token, username, role).
3. Updates Tauri backend base URL.
4. User logs in → credentials saved to `savedServers` list.

### Switching to a Saved Server
1. User selects server from saved list → `switchToServer(server)`.
2. Sets `serverUrl`, `token`, `username`, `role` from saved entry.
3. Updates Tauri backend: `set_connection` + `set_token`.
4. Saves auth to Tauri secure store.
5. SSE reconnects to new server.

### Removing a Saved Server
1. `removeServer(url, username)` → filters from `savedServers` list.
2. Does NOT logout from that server (tokens not revoked).

## ⚠️ BUG: Stale Tokens in Saved Servers

When switching to a saved server, the stored tokens may be expired. The GUI uses them directly without validation. If the access token is expired:
- First API call fails → refresh attempted → may work if refresh token is still valid.
- If both expired → auto-logout (with our fix).

But the UX is poor: user switches server, briefly sees "logged in" state, then gets kicked to login.

## ⚠️ BUG: Refresh Uses savedServers[0] Only

`tryRefreshToken()` always reads `state.savedServers?.[0]?.refresh_token`. If the user switched to a different server (not index 0), the refresh uses the wrong server's token. This could cause silent auth failures.

See `backlog/refresh-uses-wrong-server.md`.
