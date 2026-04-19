# Flow: File Attachments

## Upload
1. `POST /api/tasks/{id}/attachments` with binary body (`application/octet-stream`).
2. `X-Filename` header for original filename.
3. Max 10MB per file.
4. Filename sanitized: only alphanumeric, `.`, `-`, `_`. Leading dots stripped.
5. Storage key: `{sha256_hex_8chars}_{sanitized_name}`.
6. Written to `~/.local/share/poflow/attachments/`.
7. DB record: task_id, user_id, filename, mime_type, size, storage_key.
8. **No task ownership check on upload** — any user can attach to any task.

## Download
1. `GET /api/attachments/{id}/download`.
2. **Access check**: task owner, task assignee, or root. Non-owners/non-assignees get `403`.
3. XSS prevention: non-safe MIME types forced to `application/octet-stream`.
4. Safe types: `image/*`, `application/pdf`, `text/plain`.
5. `Content-Disposition: attachment` header forces download.

## Delete
1. `DELETE /api/attachments/{id}`.
2. Ownership check: attachment uploader or root.
3. Soft-deletes DB record. File remains on disk until orphan cleanup.

## List
- `GET /api/tasks/{id}/attachments` — no ownership check.

## Orphan Cleanup (daily background task)
Scans attachment directory, deletes files not referenced in DB.

## ⚠️ BUG: No Ownership Check on Upload
Any user can upload attachments to any task. The download is gated, but the upload is not.

## ⚠️ Note: Download Auth Is Stricter Than Other Task Operations
Download checks owner OR assignee. This is the only endpoint that grants assignees explicit access. Inconsistent with the rest of the codebase where assignees have no special permissions.
