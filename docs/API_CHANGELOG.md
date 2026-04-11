# API Changelog

## v7 (Current)

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
