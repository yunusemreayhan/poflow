# Flow: Admin Backup & Restore

## Create Backup (`POST /api/admin/backup`)
1. **Root only.**
2. `VACUUM INTO` creates a consistent copy at `~/.local/share/poflow/backups/poflow_{timestamp}.db`.
3. File permissions set to `0600`.
4. Retains last 10 backups, deletes older ones.
5. Returns `{path, size_bytes}`.

## List Backups (`GET /api/admin/backups`)
- **Root only.**
- Lists backup files with filenames and sizes.

## Restore (`POST /api/admin/restore`)
1. **Root only.**
2. Validates filename (alphanumeric + underscore + `.db` only — prevents path traversal).
3. Creates safety backup (`pre_restore_{timestamp}.db`) before restoring.
4. Checkpoints WAL for consistency.
5. Copies backup file over current DB.
6. Returns `{restored_from, safety_backup, note: "Restart the server"}`.
7. **Requires daemon restart** to use restored DB (connection pool still points to old data).

## Health Check (`GET /api/health`)
- **No auth required** (no `Claims` extractor).
- Returns: DB status, DB size, schema version, active timers, background task health.
- Background task health: checks last heartbeat for `tick`, `snapshot`, `auto_archive`.

## ⚠️ Note: Restore Doesn't Invalidate JWT Secret
If the backup was created before a JWT secret rotation, restoring it doesn't restore the old secret (which lives in a separate file). Tokens issued between backup and restore may become invalid after restart if the secret file was also affected.
