# Flow: JWT Secret Regeneration (Token Invalidation Event)

## Trigger
The `.jwt_secret` file at `~/.local/share/poflow/.jwt_secret` is deleted, corrupted (< 32 bytes), or the daemon is moved to a new machine.

## What Happens

1. Daemon starts (or first auth request on lazy init).
2. `secret()` tries to read `.jwt_secret` → file missing or too short.
3. New 64-byte random secret generated and written.
4. **All previously issued tokens are now invalid** — they were signed with the old secret.

## Impact on Users

### Immediate
- All active SSE connections continue (ticket already consumed) until they disconnect.
- On next API call or SSE reconnect: `401 Unauthorized` (signature mismatch).

### GUI Behavior
1. API call fails with 401.
2. `tryRefreshToken()` → sends old refresh token → `401` (also signed with old secret).
3. Refresh fails → auto-logout (with our fix) or stuck in limbo (without).
4. User must re-login with username/password.

### Data Impact
- No data loss — DB is unaffected.
- Token blocklist becomes irrelevant (old hashes for old tokens).

## When This Happens
- Backup restore that doesn't include `.jwt_secret`.
- Manual deletion of the poflow data directory.
- Daemon reinstall without preserving data dir.
- AI agents running tests that wipe/recreate the data dir.

## Prevention
- Set `POFLOW_JWT_SECRET` env var for stable secret across restarts.
- Include `.jwt_secret` in backup/restore procedures.
