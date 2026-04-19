# API Changelog

## v11 (Current)

### New Endpoints
- `GET /api/admin/backups` — List available database backups.
- `POST /api/admin/restore` — Restore from a named backup file.
- `GET /api/reports/user-hours` — User hours report (root only).

### Improvements
- FTS5 full-text search index on tasks (with LIKE fallback).
- Timer start uses per-task `work_duration_minutes` override.
- Attachment download verifies task ownership.
- SSE timer uses in-memory state instead of DB query per tick.
- Sprint update validates `end_date >= start_date`.
- SSE/WS ticket creation limited to 5 per user.
- Configurable `auto_archive_days` in config.
- WebSocket heartbeat monitoring (60s timeout).

## v10

### New Endpoints
- `POST /api/import/tasks/json` — Bulk import tasks from nested JSON tree.
- `POST /api/sprints/{id}/carryover` — Create new sprint with incomplete tasks.
- `PUT /api/sessions/{id}/note` — Update session notes after completion.
- `GET /api/rooms/{id}/export` — Export room estimation history as JSON.
- `POST /api/tasks/{id}/watch` — Subscribe to task updates.
- `DELETE /api/tasks/{id}/watch` — Unsubscribe from task updates.
- `GET /api/tasks/{id}/watchers` — List task watchers.
- `GET /api/watched` — List user's watched task IDs.

### Schema Changes
- `sprints.capacity_hours` — Optional team capacity in hours.
- `tasks.work_duration_minutes` — Optional per-task work session duration.
- `task_watchers` table — Task subscription tracking.

### Other
- `/api/tasks/full` now includes `labels` array.
- `/api/health` includes `schema_version`.
- Backup retention: only last 10 backups kept.
- Auto-archive: completed tasks older than 90 days archived daily.

## v9

### New Endpoints
- `GET /api/tasks/{id}/sessions` — List sessions for a task (max 200).

### Security Fixes
- Rate limiter now prunes stale IPs (prevents unbounded memory growth).
- Claims extractor caches verified user IDs for 60s (reduces per-request DB load).
- Attachment download forces safe MIME types (HTML/SVG → application/octet-stream).
- WebSocket room endpoint verifies room membership before allowing connection.

### Bug Fixes
- Sprint/epic burndown snapshots now use `remaining_points` (story points) instead of `estimated` (poflow count).
- Break sessions created by auto-start no longer associate with the previous work task.
- `daily_completed` always refreshed from DB on `get_state` (was stale when status != Idle).
- Room tasks scoped to room members when no project set (was fetching all global tasks).
- CSV export formula-prefixed fields now properly quoted.
- Sprint update defensively passes None for status to DB.

### Validation Improvements
- `PUT /api/profile` — Password change requires `current_password` field.
- `POST /api/tasks/{id}/time` — Rejects soft-deleted tasks.
- `POST /api/teams` — Limited to 50 teams.
- `POST /api/epics` — Limited to 100 epic groups.

### Cleanup
- `is_owner_or_root` wrapper replaced with re-export.
- `UserConfig` struct moved from burns.rs to types.rs.
- Dead `get_today_completed` function removed.

### DevOps
- `/api/health` now includes background task heartbeats.
- Daily orphaned attachment file cleanup.

---

## v8

### Security Fixes
- `POST /api/sprints/{id}/roots` — Requires sprint ownership (was unauthenticated).
- `DELETE /api/sprints/{id}/roots/{task_id}` — Requires sprint ownership.
- `DELETE /api/admin/users/{id}` — Deleted user's tokens are immediately invalidated.
- `GET /api/export/sessions` — Root users can now export all sessions (was always filtered to own).
- Attachment storage keys now include atomic counter to prevent collision.
- SSE ticket generation no longer panics if `/dev/urandom` is unavailable.

### Validation Improvements
- `POST /api/sprints/{id}/tasks` — Rejects soft-deleted tasks.
- `POST /api/sprints/{id}/burn` — Validates task exists and is not soft-deleted.
- `POST /api/rooms/{id}/start-voting` — Rejects soft-deleted tasks.
- `PUT /api/config` — Validates `theme` field (must be "dark" or "light").
- `POST /api/templates` — Name max 200 chars, data max 64KB, limit 100 per user.

### Bug Fixes
- `POST /api/import/tasks` — Response field is `created` (not `imported`). Frontend now reads the correct field.
- Token auto-refresh on 401 now works correctly (was referencing wrong store field).

## v7

### New Endpoints
- `GET /api/health` — Health check (no auth). Returns DB status and active timer count.
- `POST /api/tasks/{id}/restore` — Restore a soft-deleted task and its descendants.

### Breaking Changes
- `DELETE /api/tasks/{id}` now performs soft delete (sets `deleted_at`). Tasks are hidden from list queries but remain in the database. Use `/restore` to undelete.
- Task JSON response now includes `deleted_at: string | null` field.

### Security Fixes
- `GET /api/history` — Non-root users restricted to their own sessions.
- `PUT /api/tasks/{id}/labels/{label_id}` — Requires task ownership.
- `DELETE /api/tasks/{id}/labels/{label_id}` — Requires task ownership.
- `POST /api/tasks/{id}/dependencies` — Requires task ownership.
- `DELETE /api/tasks/{id}/dependencies/{dep_id}` — Requires task ownership.
- `PUT /api/tasks/{id}/recurrence` — Requires task ownership.
- `DELETE /api/tasks/{id}/recurrence` — Requires task ownership.
- `DELETE /api/templates/{id}` — Requires template ownership.

### Validation Improvements
- `POST /api/sprints/{id}/burn` — Rejects burns on completed sprints.
- `POST /api/rooms/{id}/vote` — Rejects votes when room is not in "voting" state.
- `PUT /api/tasks/{id}/recurrence` — Validates `next_due` as YYYY-MM-DD format.
- `POST /api/webhooks` — Validates event names against known list.
- `POST /api/rooms` — Limits active rooms to 20 per user.
- `POST /api/tasks/{id}/comments` — Validates task exists.
- `POST /api/tasks/{id}/attachments` — Validates task exists.
