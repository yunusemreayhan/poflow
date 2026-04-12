# BACKLOG v18 — Fresh Codebase Audit (2026-04-12)

Full audit of 56 backend .rs files (6523 LOC), 53 frontend .ts/.tsx files (9261 LOC), 275 backend tests, 154 frontend tests.

## Security (7 items)

- [x] **S1.** `health` endpoint computes `migration_version`, `active_timers`, `tasks` heartbeat map, and `db_size` but returns only `{status, db}` — dead code wastes CPU on every health check. Remove the unused computations or gate behind `?verbose=1` with auth.
- [x] **S2.** `create_backup` uses `format!("VACUUM INTO '{}'", path_str)` with single-quote escaping — still vulnerable to path injection if `POMODORO_DATA_DIR` contains crafted values. Use parameterized approach or validate path characters.
- [x] **S3.** `seed_root_user` creates user "root" with password "root" and bcrypt cost 12 — the default password bypasses `validate_password` (no uppercase/digit). Should generate a random password and print it, or refuse to start without `POMODORO_ROOT_PASSWORD`.
- [x] **S4.** `token_hash` truncates SHA-256 to 128 bits (16 bytes) — reduces collision resistance. Use full 256-bit hash for token blocklist keys.
- [x] **S5.** Attachment `upload_attachment` reads entire file into memory (`Bytes`) before writing — a 10MB upload holds 10MB in RAM. Use streaming write with `axum::body::Body` to reduce memory pressure.
- [x] **S6.** `TaskAttachments` component uses `fetch()` directly with `serverUrl + path` bypassing the Tauri `invoke("api_call")` abstraction — this leaks the auth token to the browser's network layer and bypasses CSRF protection (`x-requested-with` header). Route through Tauri IPC or add the header.
- [x] **S7.** `matchSearch` in `utils.ts` accepts regex via `/pattern/` syntax — user-controlled regex can cause ReDoS. Add a timeout or disallow regex in search.

## Bugs (18 items)

- [x] **B1.** `App.tsx` — `timerRunning` is declared inside `App()` but referenced in `Sidebar()` which is a separate component — `timerRunning` is not in scope for `Sidebar`. The sidebar timer indicator dot never shows.
- [x] **B2.** `kick_member` in rooms.rs doesn't call `engine.notify(ChangeEvent::Rooms)` — kicked user's UI won't update until next poll.
- [x] **B3.** `delete_user` reassigns `audit_log.user_id` to root but the `audit_log` JOIN in `list_audit` uses `JOIN users u ON a.user_id = u.id` — if no other root exists (edge case: deleting second-to-last root), the UPDATE subquery returns NULL and the INSERT fails.
- [x] **B4.** `carryover_sprint` creates a new sprint but doesn't copy `start_date`/`end_date` — the carry-over sprint has no dates, making burndown charts useless.
- [x] **B5.** `export_tasks` CSV doesn't include `description` column — data loss on round-trip import/export since `import_tasks_csv` also doesn't handle description.
- [x] **B6.** `TemplateManager` passes `data` as a string (`JSON.stringify({title, priority, estimated})`) but `CreateTemplateRequest` expects `data: serde_json::Value` — the backend receives a JSON string instead of a JSON object, causing double-encoding.
- [x] **B7.** `CommentSection` uses `confirm("Delete this comment?")` — native dialog, inconsistent with the rest of the app which uses `showConfirm`.
- [x] **B8.** `TaskAttachments` uses `confirm("Delete this attachment?")` — same native dialog issue.
- [x] **B9.** `Rooms` component uses `confirm("Delete this room?")` — same native dialog issue.
- [x] **B10.** `purge_task` only deletes the target task but not its soft-deleted descendants — orphaned child tasks remain in DB with `deleted_at` set but parent gone.
- [x] **B11.** `update_task` fetches the full task via `get_task` (with JOIN) just to read current values for the UPDATE — wasteful when only changing one field. Not a correctness bug but causes unnecessary DB load.
- [x] **B12.** `get_room_state` fetches all room votes with `LIMIT 500` but doesn't filter by `deleted_at IS NULL` on the tasks JOIN — vote history may reference soft-deleted tasks.
- [x] **B13.** `list_tasks_paged` with `assignee` filter uses JOIN but the `count_tasks` equivalent also JOINs — if a task has multiple assignees matching the same username (impossible due to PK), count would be wrong. Not a real bug but the JOIN pattern differs from the non-assignee path.
- [x] **B14.** `SprintParts.tsx` `Column` component has `useCallback` that captures `touchDrag` state — stale closure on touch drag operations (carried from v17 B17).
- [x] **B15.** `recurrence` processing in `main.rs` uses `today` for idempotency check (`last_created == today`) but `today` is computed once at loop start — if the loop runs across midnight, it uses stale date.
- [x] **B16.** `get_user_id_by_username` returns `Result<i64>` but callers in `kick_member` and `add_assignee` map the error to NOT_FOUND — if the DB connection fails, the user gets "User not found" instead of a 500.
- [x] **B17.** `image` preview in `TaskAttachments` uses `useStore.getState().serverUrl` directly in `<img src>` — no auth header, so the image request will fail with 401 if the server requires auth on attachment downloads.
- [x] **B18.** `due_date` reminder loop in `main.rs` only notifies via desktop notification (`notify_due_task`) but doesn't create in-app notifications — users who disable desktop notifications miss due date warnings entirely.

## Business Logic (8 items)

- [ ] **BL1.** `add_dependency` doesn't check for circular dependencies — A depends on B, B depends on A is allowed, which would permanently block both tasks.
- [ ] **BL2.** `delete_sprint` cascades via FK but doesn't clean up `sprint_root_tasks` explicitly — FK cascade handles it, but no audit log entry is created for sprint deletion.
- [ ] **BL3.** `update_task` auto-unblock logic checks ALL dependencies are completed, but doesn't handle the case where a dependency is soft-deleted — a deleted dependency should be treated as resolved.
- [ ] **BL4.** `accept_estimate` for "points" unit sets both `estimated` (integer) and `remaining_points` (float) to the same value — but `estimated` is truncated to i64, losing decimal precision for fractional story points.
- [ ] **BL5.** `log_burn` auto-assigns user to task via `INSERT OR IGNORE INTO task_assignees` — this is a side effect that may surprise users who just want to log time without being assigned.
- [ ] **BL6.** `create_room` auto-joins creator as admin but `join_room` uses `INSERT OR IGNORE` with role 'voter' — if creator leaves and re-joins, they become a voter instead of admin.
- [ ] **BL7.** `snapshot_sprint` counts `remaining_points` as "total_points" and "done_points" — but `remaining_points` is supposed to decrease as work progresses, so the burndown logic is inverted (should use `estimated` minus `remaining_points` for done).
- [ ] **BL8.** `velocity` query LEFT JOINs `sprint_tasks` and `tasks` but counts `DISTINCT CASE WHEN t.status IN ('completed','done') THEN t.id END` — tasks that appear in multiple sprints are counted once per sprint, which is correct, but cancelled burns still affect the points/hours totals even though they're filtered.

## Validation (6 items)

- [ ] **V1.** `create_room` doesn't validate `project` length — unbounded string stored in DB.
- [ ] **V2.** `create_sprint` doesn't validate `project` length — same issue.
- [ ] **V3.** `add_comment` doesn't validate `session_id` exists — can reference non-existent session.
- [ ] **V4.** `create_webhook` validates URL format but doesn't limit webhooks per user — a user could create thousands of webhooks.
- [ ] **V5.** `add_team_root_tasks` doesn't deduplicate `task_ids` — same task can be added multiple times (INSERT OR IGNORE handles it, but the count validation is wrong).
- [ ] **V6.** `import_tasks_csv` doesn't validate total task count against existing tasks — could exceed reasonable limits.

## UX Improvements (10 items)

- [ ] **UX1.** `TemplateManager` sends `data` as double-encoded JSON string — fix to send as object so templates work correctly with `instantiate_template`.
- [ ] **UX2.** No way to edit a label name or color after creation — only create and delete.
- [ ] **UX3.** No way to edit a webhook URL or events after creation — only create and delete.
- [ ] **UX4.** `TaskContextMenu` "Move up/Move down" swaps sort_order values — but if two tasks have the same sort_order (common after import), the swap is a no-op.
- [ ] **UX5.** `Dashboard` component doesn't show sprint progress or upcoming due dates — it's a missed opportunity for a useful overview.
- [ ] **UX6.** `History` component loads sessions but doesn't show task path breadcrumbs even though the API returns `task_path` — the data is fetched but not displayed.
- [ ] **UX7.** No visual indicator for tasks with dependencies — users can't see at a glance which tasks are blocked.
- [ ] **UX8.** `EstimationRoomView` doesn't show vote history export button prominently — the export endpoint exists but is hard to discover.
- [ ] **UX9.** `Timer` component doesn't show which task is being timed in the sidebar — only the timer tab shows the task name.
- [ ] **UX10.** No keyboard shortcut to quickly switch between timer and tasks — `Space` toggles timer but there's no quick way to jump to the task being timed.

## Accessibility (8 items)

- [ ] **A1.** `TaskContextMenu` — menu items lack `tabIndex={0}` so keyboard-only users can't navigate with Tab (only Arrow keys work via custom handler).
- [ ] **A2.** `NotificationBell` dropdown has `role="dialog"` but no focus trap — Tab key escapes the dropdown.
- [ ] **A3.** `Select` component — when closed, the button doesn't announce the selected value to screen readers (missing `aria-label` with current value).
- [ ] **A4.** `Timer` component — the circular progress SVG has no `role="progressbar"` or `aria-valuenow` — screen readers can't announce timer progress.
- [ ] **A5.** `EpicBurndown` — epic group chips use `div` with `role="button"` but no keyboard handler for Space key (only Enter works via `onKeyDown`).
- [ ] **A6.** `BurndownView` chart — `ResponsiveContainer` renders an SVG with no `aria-label` — screen readers see an unlabeled graphic. The `sr-only` table helps but the chart itself should have `role="img"`.
- [ ] **A7.** `CsvImport` drag-and-drop zone has no keyboard alternative — users who can't drag need the file input (which exists but is visually hidden inside the drop zone).
- [ ] **A8.** `TeamManager` — team selection buttons don't indicate current selection to screen readers (missing `aria-pressed` or `aria-current`).

## Performance (5 items)

- [ ] **P1.** `get_room_state` fetches ALL tasks for room members when no project filter — `LIMIT 500` but still loads full task rows with JOINs. Should only load leaf tasks or tasks relevant to estimation.
- [ ] **P2.** `loadTasks` in store.ts compares tasks with `some((t, i) => t.id !== prev[i]?.id || t.updated_at !== prev[i]?.updated_at)` — O(n) comparison on every load. Could use the ETag from `/api/tasks/full` to skip entirely.
- [ ] **P3.** `get_task_detail` recursive CTE fetches all descendants then batch-loads comments and sessions — but for deeply nested trees (50+ levels), the CTE can be slow. Add a depth limit parameter.
- [ ] **P4.** `Sidebar` component calls `apiCall("GET", "/api/me/teams")` on every render (no dependency array issue, but the effect runs once) — should be cached in the store.
- [ ] **P5.** `snapshot_epic_group` calls `get_descendant_ids` which runs a recursive CTE, then builds a dynamic IN clause — for large trees, this generates very long SQL. Consider using a temp table.

## Infrastructure (3 items)

- [ ] **INF1.** No database migration rollback mechanism — if a migration fails partway, the DB can be in an inconsistent state. Add a `schema_migrations.status` column to track partial migrations.
- [ ] **INF2.** `connect_memory()` used in tests doesn't run `seed_root_user` with the same password validation — test root user has password "root" which wouldn't pass `validate_password`. Tests may not catch password validation regressions.
- [ ] **INF3.** No health check for background tasks beyond heartbeat — if the tick loop panics (which `tokio::spawn` would silently swallow), the timer stops working but health endpoint still reports "ok".

---

**Total: 65 items** — S:7, B:18, BL:8, V:6, UX:10, A:8, P:5, INF:3
