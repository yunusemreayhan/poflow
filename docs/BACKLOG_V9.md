# Backlog v9 — pojidora

Generated: 2026-04-12
Backend: 197 tests | Frontend: 154 tests | TS strict: clean
Previous: v8 (63/63 items completed)

---

## Security (S1–S5)

- **S1** — `routes/mod.rs`: Rate limiter HashMap grows unbounded. `AUTH_LIMITER` and `API_LIMITER` never prune expired entries. Over time, the HashMap accumulates stale IPs. Add periodic cleanup (e.g., every 1000 insertions, remove entries older than `window_secs`).
- **S2** — `auth.rs:Claims` extractor: DB lookup on every authenticated request (`SELECT id FROM users WHERE id = ?`) adds latency. Should cache recently-verified user IDs with a short TTL (e.g., 60s) to avoid per-request DB hit.
- **S3** — `routes/attachments.rs:upload_attachment`: No MIME type validation — any content-type header is accepted and stored. Malicious files (e.g., HTML with scripts) could be served back via `download_attachment`. Should validate against an allowlist or force `application/octet-stream` on download.
- **S4** — `routes/rooms.rs:room_ws`: WebSocket endpoint authenticates via ticket but doesn't verify the user is a room member. Any authenticated user can observe any room's real-time state via WebSocket.
- **S5** — `webhook.rs:dispatch`: Webhook secret is stored as plaintext in the DB. Should be hashed or encrypted at rest. Also, webhook URLs are validated on dispatch but not on creation — a user could register a webhook pointing to an internal IP that later resolves differently.

## Bugs (B1–B8)

- **B1** — `engine.rs:tick()`: When `auto_start` is true and a session completes, the new session is created with the old `task_id` even if the phase changed from Work→Break. Break sessions shouldn't be associated with a task.
- **B2** — `engine.rs:get_state()`: `daily_completed` is only refreshed from DB when `status == Idle`. If a user's session completes on another server instance (or via DB manipulation), the in-memory count becomes stale until the user stops their timer.
- **B3** — `db/rooms.rs:get_room_state()`: The `tasks` query for rooms without a project fetches all leaf tasks globally (`WHERE t.id NOT IN (SELECT DISTINCT parent_id ...)`). This is expensive and returns unrelated tasks. Should scope to tasks owned by room members or recently voted tasks.
- **B4** — `routes/sprints.rs:update_sprint`: The `status` field is rejected with an error message, but the field is still passed to `db::update_sprint` as `req.status.as_deref()` (which is `Some(...)` when provided). The early return prevents this, but if the check were removed, the status would be silently applied. Defensive: pass `None` explicitly.
- **B5** — `store/store.ts:loadTasks`: The F10 status change notification compares against `get().allAssignees` which contains the *previous* load's assignees. If a user was just assigned and the task status changed in the same load, the notification won't fire.
- **B6** — `routes/export.rs:escape_csv`: The formula-injection prefix (`'`) is added inside the field but the field isn't then quoted. A field like `=SUM(A1)` becomes `'=SUM(A1)` but without quotes, the `'` is literal CSV content, not a spreadsheet protection. Should wrap prefixed fields in quotes.
- **B7** — `db/sprints.rs:snapshot_sprint`: Uses `estimated` (pomodoro count) as `total_points` and `done_points`. But `remaining_points` is the actual story points field. Burndown chart shows pomodoro estimates instead of story points.
- **B8** — `components/TaskDetailParts.tsx:TaskTimeChart`: The `Session` type import is from `../store/api` but the component uses `s.started_at` and `s.duration_s` which are `string` and `Option<i64>` respectively. If `started_at` is missing or `duration_s` is null, the chart silently skips entries without indication.

## Validation (V1–V5)

- **V1** — `routes/burns.rs:log_burn`: No validation that `task_id` exists or is not soft-deleted before logging a burn entry. A burn can be logged against a deleted or nonexistent task.
- **V2** — `routes/comments.rs:add_comment`: No length limit on comment content. A user could POST a multi-megabyte comment string.
- **V3** — `routes/profile.rs:update_profile`: Password change doesn't require the current password. Any authenticated user (or someone with a stolen token) can change the password without knowing the old one.
- **V4** — `routes/teams.rs:create_team`: No validation on team name length or characters. No limit on number of teams.
- **V5** — `routes/epics.rs:create_epic_group`: No validation on epic group name length. No limit on number of epic groups per user.

## Performance (P1–P4)

- **P1** — `db/rooms.rs:get_room_state()`: Fetches ALL votes for the room, then filters in Rust. For rooms with many historical votes, this loads unnecessary data. Should limit to current task + recent history.
- **P2** — `db/tasks.rs:list_tasks_paged`: The `assignee` filter does a separate query to get task IDs, then uses `IN (...)` clause. For users assigned to many tasks, this generates a huge IN list. Should use a JOIN instead.
- **P3** — `routes/misc.rs:get_tasks_full`: The ETag computation runs 5 separate COUNT/MAX queries. Should combine into a single query (partially done but could be optimized further with a materialized view or cache).
- **P4** — `engine.rs:tick()`: Drops and re-acquires the states lock to fetch per-user configs. For many concurrent users, this creates lock contention. Could pre-fetch configs before acquiring the lock.

## Code Quality (Q1–Q6)

- **Q1** — `components/TaskList.tsx` (705 LOC): Still the largest component. The inline task editing (title, description), time reporting, and comment sections are all embedded. Extract `TaskInlineEdit` and `TaskTimeReportInline` components.
- **Q2** — `components/TaskDetailView.tsx` (582 LOC): Large component with export, rollup, inline editing, labels, dependencies, recurrence, attachments, and activity feed all in one file. Could extract the header/metadata section.
- **Q3** — `components/Settings.tsx` (434 LOC): Contains AdminPanel, UserConfig, TeamManager, and main Settings all in one file. AdminPanel and TeamManager could be extracted.
- **Q4** — `db/mod.rs:migrate()` (393 LOC): The migration function contains all CREATE TABLE statements inline. As the schema grows, this becomes unwieldy. Consider splitting into numbered migration files or at least separate functions per table group.
- **Q5** — `routes/rooms.rs` (207 LOC) + `db/rooms.rs` (177 LOC): The `accept_estimate` route contains complex auto-advance logic (find next unestimated leaf task). This business logic should be in the engine or a service layer, not in the route handler.
- **Q6** — `store/store.ts` (399 LOC): The store mixes auth, timer, task, UI state, and toast logic. Could split into slices using Zustand's `combine` or separate stores.

## Features (F1–F12)

- **F1** — Batch task operations: Add bulk delete, bulk assign, bulk move (change parent). Currently only bulk status change exists.
- **F2** — Task search in sprint backlog view: The sprint backlog shows all available tasks but has no search/filter. Large task lists are hard to navigate.
- **F3** — Sprint velocity chart: The `/api/sprints/velocity` endpoint exists but there's no frontend visualization. Add a velocity chart to the Sprints tab showing points/hours per completed sprint.
- **F4** — Task time report history: The inline time report only shows total hours. Add a collapsible list of individual burn entries with dates and users.
- **F5** — Room WebSocket auto-reconnect: The `EstimationRoomView` uses polling via SSE debounce. The WebSocket endpoint exists (`/api/rooms/{id}/ws`) but the frontend doesn't use it. Switch to WebSocket for real-time room updates.
- **F6** — Task activity timeline: Show a chronological feed of all changes to a task (status changes, assignee changes, comments, burns) in the task detail view. The audit log has this data but it's not surfaced per-task.
- **F7** — Attachment preview: Currently attachments can only be downloaded. Add inline preview for images and text files in the task detail view.
- **F8** — Sprint retrospective view: The `retro_notes` field exists but there's no dedicated UI for writing/viewing sprint retrospectives. Add a retro tab to the sprint detail.
- **F9** — Keyboard shortcut for task status cycling: Allow pressing a key (e.g., `s`) on a focused task row to cycle through statuses (backlog → active → in_progress → done).
- **F10** — Export sprint report: Generate a summary report (markdown or PDF) for a completed sprint including burndown chart, velocity, task completion stats, and per-user breakdown.
- **F11** — Task templates from existing tasks: Allow saving an existing task (with subtasks) as a template, not just creating templates from scratch.
- **F12** — Notification preferences per event type: Currently `notify_desktop` and `notify_sound` are global toggles. Allow per-event configuration (e.g., notify on session complete but not on due date reminders).

## UX (U1–U8)

- **U1** — Timer task selector: The task dropdown shows all active/backlog tasks in a flat list. For users with many tasks, this is hard to navigate. Add search/filter to the task selector.
- **U2** — Sprint board drag-and-drop between columns: The board view shows todo/in_progress/done columns but tasks can only be moved by clicking status buttons. Add drag-and-drop between columns.
- **U3** — Task detail breadcrumb navigation: When viewing a deeply nested task, there's no way to navigate up the parent chain. Add breadcrumbs showing the task hierarchy path.
- **U4** — Estimation room countdown timer: When all votes are in, add an optional 3-second countdown before auto-reveal to build anticipation and give last-second voters a chance.
- **U5** — Empty state illustrations: Several views (no tasks, no sprints, no rooms, no history) show plain text. Add simple SVG illustrations or icons for empty states.
- **U6** — Task due date visual indicators: Tasks with due dates don't have visual urgency indicators. Add color coding (red for overdue, yellow for due today/tomorrow, green for upcoming).
- **U7** — Sidebar badge counts: Show unread notification count or active timer indicator on sidebar tab icons.
- **U8** — Settings save confirmation: The config save button shows "Saving..." briefly but doesn't confirm success. Add a checkmark animation or "Saved!" feedback.

## Accessibility (A1–A5)

- **A1** — `components/TaskList.tsx`: Task rows use `onContextMenu` for the context menu but there's no keyboard equivalent. Right-click menus are inaccessible to keyboard-only users. Add a menu button or Shift+F10 handler.
- **A2** — `components/Sprints.tsx`: Sprint board columns don't have ARIA roles. Should use `role="list"` on columns and `role="listitem"` on task cards for screen reader navigation.
- **A3** — `components/EstimationRoomView.tsx`: Vote cards don't announce selection state to screen readers. The selected card should have `aria-pressed="true"` and the card grid should have `role="radiogroup"`.
- **A4** — `components/History.tsx:HeatmapCell`: The heatmap uses `role="gridcell"` but the parent container doesn't have `role="grid"` with proper row/column structure.
- **A5** — `components/Timer.tsx`: The circular progress SVG doesn't have a text alternative describing the current progress percentage for screen readers.

## Tests (T1–T8)

- **T1** — No tests for WebSocket room endpoint (`/api/rooms/{id}/ws`). Should test connection, state broadcast, and disconnection.
- **T2** — No tests for the rate limiting middleware. Should verify that requests beyond the limit return 429, and that GET requests are not limited.
- **T3** — No tests for attachment upload/download flow. Should test size limits, filename sanitization, and MIME type handling.
- **T4** — No tests for the recurring task processor. Should test daily/weekly/monthly patterns and idempotency.
- **T5** — No tests for the webhook dispatch system. Should test SSRF protection (private IP blocking), retry logic, and HMAC signature generation.
- **T6** — No frontend tests for the SSE connection hook (`useSseConnection`). Should test reconnection logic and event handling.
- **T7** — No tests for CSV import edge cases: quoted fields with commas, escaped quotes, formula injection prevention, empty lines.
- **T8** — No tests for the sprint burndown snapshot accuracy. Should verify that snapshot correctly aggregates task points/hours.

## Documentation (D1–D3)

- **D1** — No CONTRIBUTING.md or development setup guide. New contributors have no instructions for building, testing, or running the project locally.
- **D2** — The API changelog (`docs/API_CHANGELOG.md`) doesn't document the v9 endpoints added in v8 (task sessions, task time chart, etc.).
- **D3** — No documentation for the WebSocket protocol used by estimation rooms. The message format and event types are undocumented.

## DevOps (O1–O3)

- **O1** — No database backup mechanism. The SQLite database at `~/.local/share/pomodoro/pomodoro.db` has no automated backup. Add a `/api/admin/backup` endpoint or a CLI command.
- **O2** — No health check for background tasks. The tick loop, snapshot loop, and recurrence loop run silently. If one panics, there's no monitoring or restart. Add health status to `/api/health`.
- **O3** — Attachment storage has no cleanup for orphaned files. If the DB record is deleted but the file deletion fails, the file remains on disk forever. Add a periodic cleanup job.

## Cleanup (C1–C4)

- **C1** — `routes/types.rs`: `is_owner_or_root` is a thin wrapper that delegates to `auth::is_owner_or_root`. All callers could use `auth::is_owner_or_root` directly. Remove the wrapper.
- **C2** — `db/burns.rs`: `UserConfig` struct is defined in `burns.rs` but logically belongs in `users.rs` or `types.rs`. It's used by the engine for per-user config.
- **C3** — `hooks/useSseDebounce.ts` (10 LOC): This hook is only used in `EstimationRoomView.tsx`. It's a trivial wrapper around `useEffect` + `addEventListener`. Could be inlined.
- **C4** — Dead code: `db/sessions.rs:get_today_completed()` (no user filter) is never called — only `get_today_completed_for_user()` is used. Remove the dead function.
