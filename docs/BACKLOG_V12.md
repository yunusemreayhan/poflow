# BACKLOG v12 — pomodoroLinux

Fresh codebase audit: 5,902 LOC backend (Rust), 7,331 LOC frontend (React/TS), 229 backend tests, 154 frontend tests.

---

## Bugs (B1–B14)

- **B1: `delete_sprint` ignores `map_err` return value.** `sprints.rs:73` calls `db::delete_sprint(...).map_err(internal);` but discards the Result — a DB error is silently ignored and 204 is returned anyway. Must use `?` operator.
- **B2: FTS5 triggers fire on soft-delete UPDATE.** The `tasks_fts_update` trigger updates the FTS index even when `deleted_at` is set, meaning soft-deleted tasks remain searchable via `?search=`. The trigger should skip rows where `new.deleted_at IS NOT NULL` (or delete from FTS).
- **B3: `get_stats` ignores user scope.** `GET /api/stats` returns day stats for ALL users regardless of who's calling. Non-root users should only see their own stats.
- **B4: SSE `change` events don't include user scope.** All change events (Tasks, Sprints, Rooms) are broadcast to all connected SSE clients. A user editing their own task triggers a reload for every connected user. Should filter by relevance or add a user_id field.
- **B5: `list_rooms` returns all rooms to all users.** Any authenticated user can see every room including ones they haven't joined. Should filter to rooms the user is a member of (plus public/open rooms).
- **B6: Dashboard activity timeline never refreshes.** The audit log is fetched once on mount (`useEffect([], [])`) but never updates. Should re-fetch when the tab becomes active or on a timer.
- **B7: `user_hours_report` date filter uses string comparison.** The `started_at >= ? AND started_at <= ?` comparison works for ISO dates but the `from`/`to` params aren't validated as dates — passing garbage strings silently returns wrong results.
- **B8: `restore_backup` overwrites live DB without closing connections.** The restore copies a file over the active DB while the connection pool is still open. SQLite WAL mode may cause corruption. Should close pool or use `VACUUM INTO` in reverse.
- **B9: Saved filters use `prompt()` which blocks the UI.** The `saveCurrentFilter` function in TaskList.tsx uses `window.prompt()` which is a blocking modal and doesn't work in Tauri on some platforms.
- **B10: `bulkSelected` state not declared in TaskList.** The table view references `bulkSelected` and `setBulkSelected` but these are only passed as props from the tree view — the table view's own bulk selection state is missing when `selectMode` is false.
- **B11: `InlineTimeReport` uses `confirm()` dialog.** `TaskInlineEditors.tsx` uses `window.confirm()` which is blocking and inconsistent with the app's custom confirm dialog pattern.
- **B12: FTS5 not available in all SQLite builds.** The migration creates a virtual FTS5 table but doesn't handle the case where SQLite was compiled without FTS5 support. Should catch the error and fall back to LIKE queries.
- **B13: `auto_archive_days` not exposed in frontend Config type.** The backend Config has `auto_archive_days` but the frontend `Config` interface in `types.ts` doesn't include it, so it can't be configured from Settings.
- **B14: `export_burns` has no ownership check.** Any authenticated user can export burn logs for any sprint via `GET /api/export/burns/{sprint_id}` without verifying sprint ownership or membership.

## Security (S1–S5)

- **S1: `add_task_dependency` has no ownership check.** Any user can add dependencies to any task — should verify the caller owns the task or is root.
- **S2: `remove_task_dependency` has no ownership check.** Same issue — any user can remove dependencies from any task.
- **S3: `get_task_blocking` exposes dependency info for any task.** Should verify the caller has access to the task (owner, assignee, or root).
- **S4: Webhook secret displayed in `list_webhooks` response.** The encrypted secret blob is returned in the webhook list — should be omitted or masked.
- **S5: `user_hours_report` date params not sanitized.** The `from` and `to` query params are passed directly to SQL without date format validation. While parameterized (safe from injection), invalid values produce silent wrong results.

## Validation (V1–V6)

- **V1: `create_sprint` allows duplicate names.** Multiple sprints can have the same name which causes confusion in the velocity chart and carry-over naming.
- **V2: `add_dependency` allows duplicate dependencies.** The `task_dependencies` table has a PK constraint but the error is returned as a 500 instead of a friendly 409/400.
- **V3: `cast_vote` allows non-voter role members to vote.** Room members with role "observer" can still cast votes — should check member role.
- **V4: `import_tasks_csv` has no description length limit.** The CSV import doesn't validate field lengths, allowing arbitrarily long titles/descriptions that bypass the create_task validation.
- **V5: `capacity_hours` has no upper bound.** Sprint capacity_hours accepts any f64 including negative values and infinity.
- **V6: `log_burn` allows negative hours/points.** The burn log endpoint doesn't validate that hours and points are non-negative.

## Tests (T1–T10)

- **T1: FTS5 search integration test.** Create tasks, search via `?search=`, verify FTS5 MATCH returns correct results and excludes soft-deleted tasks.
- **T2: Backup list and restore endpoints.** Test `GET /api/admin/backups` returns list, `POST /api/admin/restore` with invalid filename returns 400.
- **T3: User hours report.** Test `GET /api/reports/user-hours` returns correct aggregation, non-root gets 403.
- **T4: Sprint carry-over preserves capacity.** Verify the new sprint from carry-over inherits `capacity_hours` from the original.
- **T5: Dependency ownership enforcement.** Test that non-owner can't add/remove dependencies (after S1/S2 fix).
- **T6: Room membership filter.** Test that `list_rooms` only returns rooms the user is a member of (after B5 fix).
- **T7: Bulk status update with mixed ownership.** Test that `bulk-status` rejects when some tasks belong to other users.
- **T8: CSV import field length validation.** Test that CSV import rejects titles > 500 chars (after V4 fix).
- **T9: Sprint date ordering on create.** Test that `create_sprint` rejects `end_date < start_date`.
- **T10: Concurrent timer start.** Test that starting a timer while one is running properly ends the previous session.

## Performance (P1–P4)

- **P1: `get_room_state` fetches all votes for title lookup.** The vote history section does a batch title lookup query even when there are no historical votes. Should skip the query when `voted_tids` is empty (already guarded but the `HashSet` allocation is unnecessary).
- **P2: `list_tasks_paged` team scope uses recursive CTE per request.** The `get_descendant_ids` call for team-scoped queries runs a recursive CTE on every request. Should cache team scope IDs with a short TTL.
- **P3: `get_tasks_full` ETag computation runs 7 subqueries.** The ETag is computed from 7 separate COUNT/MAX queries. Should combine into a single query or use a materialized counter table.
- **P4: Dashboard fetches audit log without user filter.** `GET /api/audit?limit=10` returns the last 10 audit entries for ALL users. For non-root users, should filter to their own activity.

## Features (F1–F12)

- **F1: Task search with highlighting.** The FTS5 backend supports `snippet()` and `highlight()` functions — expose a `/api/tasks/search` endpoint that returns ranked results with highlighted matches.
- **F2: Password change endpoint.** No way to change password after registration. Add `PUT /api/auth/password` requiring current + new password.
- **F3: Sprint comparison view.** Compare two sprints side-by-side (velocity, completion rate, team hours). Frontend component + backend endpoint.
- **F4: Task time tracking summary.** Add `GET /api/tasks/{id}/time-summary` returning total hours by user, by day, and by week for a task.
- **F5: Notification preferences UI.** The `notification_prefs` table exists but there's no frontend UI to configure per-event notification settings.
- **F6: Bulk task import from Markdown.** Support pasting Markdown task lists (`- [ ] task`) with nested indentation creating parent-child relationships.
- **F7: Sprint retrospective export.** Export sprint retro notes + burndown chart data as Markdown or PDF for sharing.
- **F8: Task comment editing.** Comments can only be created and deleted — add `PUT /api/comments/{id}` for editing within a time window (e.g., 15 minutes).
- **F9: Room estimation presets.** Allow room admins to save custom card decks (e.g., T-shirt sizes: XS, S, M, L, XL) instead of only Fibonacci/hours.
- **F10: Webhook event filtering UI.** Webhooks support `events` field but the frontend has no UI to select which events trigger the webhook.
- **F11: Task template variables.** Templates currently store static JSON — support `{{today}}`, `{{username}}` variables that are resolved on instantiation.
- **F12: Multi-user timer visibility.** Show other team members' active timer status on the dashboard (who's working on what).

## UX (U1–U7)

- **U1: Timer ring animation stutters on tab switch.** The SVG ring animation uses `elapsed_s` from SSE which pauses when the tab is backgrounded. Should use `requestAnimationFrame` with server time delta.
- **U2: No visual feedback on clipboard paste import.** When pasting multiple lines into the task input (F5 from v11), there's no toast or count showing how many tasks were created.
- **U3: Sprint board cards don't show labels.** The board view shows title, hours, points, and owner but not task labels which are useful for categorization.
- **U4: History page has no date range picker.** The history component loads all sessions — should add date range filtering to reduce load and improve usability.
- **U5: Overdue tasks not sorted by urgency.** Dashboard overdue section shows tasks in arbitrary order — should sort by due date (most overdue first).
- **U6: No empty state for estimation rooms.** When there are no rooms, the Rooms tab shows nothing — should show a "Create your first room" prompt.
- **U7: Task detail view doesn't show dependencies.** The TaskDetailView shows comments, sessions, and attachments but not task dependencies (blocking/blocked-by).

## Code Quality (Q1–Q5)

- **Q1: `delete_sprint` missing `?` operator.** Same as B1 — the `map_err(internal)` result is discarded. This is both a bug and a code quality issue.
- **Q2: Duplicate `escape_csv` logic.** The CSV escape function in `export.rs` is used for both task and session exports but could be extracted to a shared utility.
- **Q3: `epic_groups.rs` uses `created_by != claims.user_id as i64`.** The cast `claims.user_id as i64` is redundant since `user_id` is already `i64`. Should use direct comparison.
- **Q4: Frontend `Config` type missing `auto_archive_days` and `cors_origins`.** The frontend type doesn't match the backend struct, causing silent field drops on config save.
- **Q5: `add_task_dependency` uses raw JSON parsing.** The handler parses `req["depends_on_id"]` from a `serde_json::Value` instead of using a typed request struct.

## Documentation (D1–D3)

- **D1: API changelog missing v11 FTS5 and restore endpoints.** The v11 entries in `API_CHANGELOG.md` don't mention the FTS5 migration, backup restore, or user hours report endpoints.
- **D2: CONTRIBUTING.md missing FTS5 requirements.** The contributing guide doesn't mention that SQLite must be compiled with FTS5 support for search to work.
- **D3: OpenAPI spec missing `import_tasks_csv` and `export_burns`.** These endpoints exist and are registered as routes but not in the OpenAPI `paths` list.

## DevOps (O1–O3)

- **O1: No database integrity check on startup.** The daemon starts without running `PRAGMA integrity_check` — a corrupted DB will cause random failures instead of a clear error at startup.
- **O2: Health endpoint doesn't report FTS5 status.** The health check should verify the FTS5 index exists and is queryable, since search silently fails if the virtual table is missing.
- **O3: No log rotation or size limit.** The daemon logs to stdout with no rotation. In production deployments without an external log manager, logs grow unbounded.

## Accessibility (A1–A3)

- **A1: Timer ring SVG has no accessible label.** The circular progress SVG in Timer.tsx has no `role` or `aria-label` — screen readers can't convey timer progress.
- **A2: Sprint board columns lack ARIA landmarks.** The board columns use `role="list"` but individual cards don't have `role="listitem"`, and there's no `aria-live` region for status changes.
- **A3: Keyboard shortcuts overlay not keyboard-dismissible.** The `?` shortcuts modal can only be closed by clicking the Close button or the backdrop — should also close on Escape key.

## Cleanup (C1–C3)

- **C1: Unused `_state` parameter in `list_backups`.** The handler takes `State(AppState)` as `_state` but never uses the engine — could be removed.
- **C2: Dead `SseQuery.token` field.** The `SseQuery` struct has a `token` field that's never used (tickets replaced tokens). Should be removed.
- **C3: `RoomMember` type has `user_id` field not in backend response.** The frontend `RoomMember` interface includes `user_id: number` but the backend `RoomMember` serialization uses `username` — the field is always 0/undefined.

---

**Total: 65 items** (14 bugs, 5 security, 6 validation, 10 tests, 4 performance, 12 features, 7 UX, 5 code quality, 3 documentation, 3 devops, 3 accessibility, 3 cleanup)
