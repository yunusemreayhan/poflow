# API Changelog

## v8 (Current)

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
