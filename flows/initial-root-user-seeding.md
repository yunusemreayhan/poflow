# Flow: Initial Root User Seeding (First Boot)

## Trigger
Daemon starts and `db::connect()` is called.

## Steps

1. `db::connect()` opens/creates SQLite DB at `~/.local/share/poflow/poflow.db`.
2. `migrate()` runs — creates all tables if they don't exist.
3. `seed_root_user()` runs:
   - Queries `SELECT COUNT(*) FROM users`.
   - **Only if count == 0** (empty DB):
     - Reads `POFLOW_ROOT_PASSWORD` env var (default: `"root"`).
     - Hashes password with bcrypt cost 12.
     - Inserts user `root` with role `root`.
     - Logs warning if using default password.
4. If count > 0, seed is skipped entirely.

## JWT Secret Initialization
Happens on first API call that needs auth (lazy via `OnceLock`):
1. Check `POFLOW_JWT_SECRET` env var → use if set.
2. Check `~/.local/share/poflow/.jwt_secret` file → use if exists and ≥32 bytes.
3. Generate 64 random bytes from `/dev/urandom`, write to `.jwt_secret`, set permissions `0600`.

## Token Blocklist Initialization
`auth::init_pool()` called in `main()`:
- Loads all non-expired token hashes from `token_blocklist` table into in-memory `HashSet`.

## ⚠️ BUG: Seed Only Runs on Empty DB

If all users are deleted except one non-root user, there's no recovery path — `seed_root_user` won't run because `count > 0`. The only way to get root access back is direct DB manipulation.

See `backlog/seed-root-not-resilient.md`.

## ⚠️ BUG: Default Root Password Bypasses Validation

The seed uses password `"root"` which is 4 characters, no uppercase, no digit. This violates the `validate_password` rules (8+ chars, uppercase, digit). The seed bypasses validation because it calls `db::create_user` directly, not the `/api/auth/register` endpoint.

A user who knows the default password can login, but if they try to change it via the profile endpoint, they must meet the password policy. However, the root user can never be forced to change the weak default password.

See `backlog/default-root-password-weak.md`.
