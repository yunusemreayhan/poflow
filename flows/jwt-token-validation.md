# Flow: JWT Token Validation (Per-Request)

## Trigger
Every API request to a protected endpoint (anything with `claims: Claims` parameter).

## Steps (in `Claims::from_request_parts`)

1. **CSRF check**: For non-GET/HEAD/OPTIONS requests, require `X-Requested-With` header.
   - Missing → `403 Forbidden` (no body, raw status code).
2. **Extract token**: Read `Authorization: Bearer <token>` header.
   - Missing or malformed → `401 Unauthorized`.
3. **Revocation check**: `is_revoked(token)` — checks in-memory `HashSet` of SHA-256 hashes.
   - Revoked → `401 Unauthorized`.
4. **JWT verification**: `verify_token(token)` — validates signature and expiry using `jsonwebtoken` crate.
   - Invalid/expired → `401 Unauthorized`.
5. **Token type check**: Reject if `claims.typ == "refresh"`.
   - Refresh tokens cannot be used as access tokens → `401 Unauthorized`.
6. **Deleted user check**: Query DB to verify `users.id` still exists.
   - **Cached for 60 seconds** per user_id (in-memory HashMap).
   - Cache pruned when size > 200 entries.
   - User not found → `401 Unauthorized`.
7. Return `Claims` struct to route handler.

## Token Expiry Defaults
| Token Type | Default Expiry | Env Var Override |
|---|---|---|
| Access | 2 hours | `ACCESS_TOKEN_EXPIRY_SECS` |
| Refresh | 30 days | `REFRESH_TOKEN_EXPIRY_SECS` |

## ⚠️ BUG: Role Changes Not Reflected Until Token Expires

JWT claims include `role` at issuance time. If a root user demotes another user from `root` to `user` via `PUT /api/admin/users/{id}/role`, the demoted user's existing access token still contains `role: "root"` until it expires (up to 2 hours).

The refresh endpoint re-fetches the user from DB, so a token refresh will pick up the new role. But during the access token's lifetime, the old role is used.

See `backlog/role-change-not-immediate.md`.

## ⚠️ BUG: Deleted User Has 60-Second Grace Period

Due to the user existence cache (60s TTL), a deleted user's tokens continue to work for up to 60 seconds after deletion. The `invalidate_user_cache` call in `delete_user` mitigates this for the specific user, but only on the same daemon instance.

This is acceptable for a single-instance deployment but would be a problem in a multi-instance setup.
