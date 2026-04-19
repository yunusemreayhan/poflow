# Backlog v5 — poflow

Generated: 2026-04-11
Previous: v4 (131/146 complete), v3 (78/78), v2 (61/61)
Test baseline: 105 backend, 52 frontend

---

## Bugs (18 items)

### Backend

**B1** — `tick()` holds `states` lock while doing async DB calls (`get_user_config` per user). Under high user count, this blocks all timer operations. The user config fetch should be done before acquiring the states lock, or cached.

**B2** — `pause()` and `resume()` acquire `config` lock unnecessarily. They only need `states` — the config lock is taken just to create a default idle state that's never used if the user already exists.

**B3** — `get_state()` acquires `states` lock then `config` lock, violating the documented lock ordering (config → states). Safe today because both are short-lived, but a latent deadlock risk.

**B4** — `skip()` doesn't increment `session_count` when skipping a work phase. A skipped work session should still count toward the long break interval, otherwise the interval counter drifts.

**B5** — Webhook HMAC uses `SHA256(secret + body)` which is vulnerable to length-extension attacks. Should use proper HMAC construction: `HMAC-SHA256(secret, body)`.

**B6** — `create_sse_ticket` fallback (when `/dev/urandom` unavailable) uses `DefaultHasher` which produces predictable tickets. Should fail hard like JWT secret does (S7 from v4), or use `getrandom` crate.

**B7** — `room_ws` auto-joins the user to the room on WebSocket connect. This is a side effect of a read operation — connecting to observe shouldn't mutate membership.

**B8** — `accept_estimate` auto-advance logic filters leaf tasks by checking `!all_tasks.iter().any(|c| c.parent_id == Some(t.id))` which is O(n²) for n tasks. Should pre-compute parent set.

**B9** — `export_tasks` exports ALL tasks regardless of user. No ownership filter — any authenticated user can export every user's tasks.

**B10** — `export_sessions` date range is hardcoded to `2000-01-01` to `2099-12-31`. Should accept query parameters for date filtering.

### Frontend

**B11** — SSE fallback in `App.tsx` still tries `?token=` if ticket exchange fails (line ~195). The legacy token path was removed in v4 (S4). This fallback will always fail and should be removed.

**B12** — `__tasksLoadedAt` uses `window as unknown` cast to store a timestamp. Should be a module-level variable (Q2 from v4 missed this one).

**B13** — `Sidebar` fetches `/api/me/teams` on every render cycle because `useEffect` has no dependency on token. If token changes (re-login), teams aren't refreshed.

**B14** — `connectSse` in `App.tsx` is defined inside `useEffect` but `sseInstance` is declared with `let` — the cleanup function captures the outer `let` which may be `null` if `connectSse` hasn't resolved yet. Race condition on fast unmount.

**B15** — `TaskNode` expanded state resets when parent re-renders because `expanded` is local `useState(false)`. Expanding a child, then editing the parent title, collapses all children.

**B16** — `EstimationRoomView` countdown fires `doReveal` even if the room status changed (e.g., another admin revealed first). Should check room status before calling reveal.

**B17** — `BurnsView` doesn't validate that `taskId > 0` before submitting burn. If tasks array is empty and user submits, it sends `task_id: 0`.

**B18** — `History` heatmap assumes 365 days but doesn't account for leap years. Feb 29 data may be misaligned.

---

## Security (10 items)

**S1** — Webhook HMAC uses concatenation (`SHA256(secret + body)`) instead of proper HMAC. Vulnerable to length-extension attacks. Use `hmac` crate with `Hmac<Sha256>`.

**S2** — `auth_key()` in Tauri lib derives encryption key from hostname + username only. This is static and guessable. Should include a random salt stored alongside the auth file.

**S3** — XOR encryption for saved auth (`xor_bytes`) is trivially reversible if the key is known (and it's derived from public info). Should use a proper authenticated encryption scheme (e.g., AES-GCM via `aes-gcm` crate).

**S4** — `write_file` Tauri command allows writing to Downloads/Documents/Desktop but doesn't validate file extension. A malicious frontend could write `.desktop` files (Linux) or `.bat` files that auto-execute.

**S5** — No CORS origin validation for production deployments. The allowed origins are hardcoded to localhost. Production deployments behind a reverse proxy will fail or require disabling CORS.

**S6** — JWT access tokens have 2-hour expiry but there's no sliding window or short-lived session concept. A stolen token is valid for the full 2 hours with no way to detect misuse.

**S7** — `bcrypt` cost factor is 12 which is reasonable, but there's no mechanism to upgrade hashes when cost factor changes. Should check and rehash on login if cost differs.

**S8** — Attachment storage uses predictable filenames (`timestamp_filename`). An attacker who can guess the timestamp can construct valid storage keys. Should use random UUIDs.

**S9** — No Content-Security-Policy headers on API responses. XSS via attachment download (if mime_type is `text/html`) could execute in the API origin context.

**S10** — Rate limiter uses IP-based tracking but doesn't handle IPv6 properly. `x-forwarded-for` may contain IPv6 addresses that bypass rate limits if the limiter treats `::ffff:127.0.0.1` and `127.0.0.1` as different keys.

---

## Features (14 items)

**F1** — Task archiving. Completed tasks accumulate forever. Add an "archive" status that hides tasks from default views but preserves history. Bulk archive completed tasks older than N days.

**F2** — Sprint velocity trend line. The velocity chart shows raw data but no trend line or average. Add a moving average overlay to help predict future sprint capacity.

**F3** — Task time tracking integration. The timer auto-logs burns but there's no way to see "time spent today" across all tasks in a dashboard widget. Add a daily time summary.

**F4** — Notification preferences per event type. Currently it's all-or-nothing (desktop + sound). Users should be able to configure which events trigger notifications (work complete, break complete, due date, etc.).

**F5** — Sprint retrospective template. The retro_notes field is free-text. Add structured retro sections: "What went well", "What didn't", "Action items".

**F6** — Task import from CSV/JSON. Export exists but there's no import. Users should be able to bulk-create tasks from a file.

**F7** — Keyboard shortcut for starting timer with selected task. Currently requires mouse click on task picker then start button. Add `s` shortcut to start timer with the currently selected/viewed task.

**F8** — Dark/light theme persistence per device. Theme is stored in server config, so switching theme on desktop also changes it on another device. Should be local-only or per-device.

**F9** — WebSocket reconnection with exponential backoff. The SSE fallback polls every 2s but doesn't attempt to reconnect SSE. Should auto-reconnect SSE with backoff.

**F10** — Bulk task status change. Select multiple tasks and change status (e.g., mark all as "done"). Currently only individual updates are possible.

**F11** — Sprint task auto-snapshot on status change. Snapshots only happen hourly or on sprint start/complete. Changing a task status (backlog → done) should trigger an immediate snapshot for accurate burndown.

**F12** — Room WebSocket ping/pong keepalive. The WS connection has no heartbeat. Idle connections may be silently dropped by proxies without the client knowing.

**F13** — Export burns/time reports. Tasks and sessions can be exported but burn log data cannot. Add CSV/JSON export for burn entries.

**F14** — Configurable CORS origins via environment variable or config file. Currently hardcoded to localhost origins only.

---

## Performance (8 items)

**P1** — `tick()` calls `db::get_user_config()` for every running user every second. Should cache user configs in memory with a TTL (e.g., 60s) to avoid per-second DB queries.

**P2** — `get_state()` calls `db::get_today_completed_for_user()` on every poll. This is a DB query per second per connected client. Should cache in the `EngineState` and only refresh on session completion.

**P3** — `get_task_detail` recursively fetches children with N+1 queries. For a task with 20 children, each with comments and sessions, this is 60+ queries. Should batch-fetch all descendants in one query.

**P4** — `get_all_burn_totals` scans the entire `burn_log` table on every `/api/tasks/full` call. Should use a materialized view or summary table updated on burn insert/cancel.

**P5** — `snapshot_sprint` recalculates all task statuses every hour for every active sprint. For large sprints (100+ tasks), this is expensive. Should be incremental — only recalculate when tasks change.

**P6** — Frontend `buildTree` runs on every tasks array change. With 1000+ tasks, tree construction is noticeable. Should use Web Worker for tree building to avoid blocking the main thread.

**P7** — `list_tasks_paged` builds a dynamic SQL query with string concatenation. Should use a query builder or prepared statement fragments to avoid repeated parsing.

**P8** — SSE broadcasts timer state to all connected clients every second, even if nothing changed. Should only send on actual state transitions (start, pause, tick completion, etc.).

---

## Code Quality (12 items)

**Q1** — `TaskDetailView.tsx` is 686 lines. Extract `TaskAttachments`, `TaskActivityFeed`, `ProgressBar`, `ExportButton` into separate files.

**Q2** — `__tasksLoadedAt` stored on `window` object. Replace with a module-level variable in `store.ts`.

**Q3** — `Settings.tsx` is 518 lines with 5 sub-components inline. Extract `WebhookManager`, `TemplateManager`, `TeamManager` to separate files.

**Q4** — Duplicated password validation logic in `register`, `update_profile`, and potentially future endpoints. Extract to a shared `validate_password()` function.

**Q5** — Duplicated username validation logic in `register` and `update_profile`. Extract to `validate_username()`.

**Q6** — `engine.rs` has duplicated user config merge logic in `get_user_config()`, `tick()`, and `start()`. Extract to a single `merge_user_config(global: &Config, user: &UserConfig) -> Config` function.

**Q7** — `App.tsx` SSE setup is 50+ lines inside a `useEffect`. Extract to a custom `useSse()` hook.

**Q8** — `Sprints.tsx` is 450 lines with 5 view components. `BoardView`, `BacklogView`, `SummaryView` should be in separate files.

**Q9** — `escape_csv` in `export.rs` doesn't handle carriage returns (`\r`). CSV fields with `\r` will corrupt the output.

**Q10** — `valid_date` in `tasks.rs` only checks format, not validity. `2024-02-31` passes validation. Should use `chrono::NaiveDate::parse_from_str` for actual date validation.

**Q11** — `EpicBurndown.tsx` (166 lines) duplicates chart logic from `SprintViews.tsx`. Should share a common `BurndownChart` component.

**Q12** — `types.rs` in routes has 25+ request/response structs. Split into `request_types.rs` and `response_types.rs` for clarity.

---

## UX (12 items)

**U1** — No loading state when switching between sprint views (board, backlog, burndown). The view flashes empty before data loads.

**U2** — Sprint board columns have no task count badges. Hard to see at a glance how many tasks are in each status.

**U3** — Timer doesn't show which task is currently being timed. The task picker shows the selection but once started, there's no persistent indicator of the active task name.

**U4** — No "undo" for task deletion. The confirmation dialog exists but once confirmed, the task is permanently gone. Add a soft-delete with 30-second undo window.

**U5** — Heatmap in History view has no legend explaining what the colors mean. New users won't understand the intensity scale.

**U6** — Room voting cards don't show the Fibonacci sequence or custom scale. Users have to guess valid values. Show the scale (1, 2, 3, 5, 8, 13, 21) as preset buttons.

**U7** — No visual indicator of task dependencies in the tree view. Blocked tasks look the same as unblocked ones. Add a small icon or color indicator for tasks with unresolved dependencies.

**U8** — Sprint date range not shown in the sprint list. Users have to click into each sprint to see start/end dates.

**U9** — No empty state illustrations. Empty task list, empty sprint list, empty history all show blank space. Add helpful empty state messages with action prompts.

**U10** — Sidebar team selector truncates team names to 4 characters. Teams with similar prefixes (e.g., "Frontend", "Framework") are indistinguishable. Show tooltip on hover.

**U11** — No keyboard shortcut to navigate between sprints. Users must click. Add `[` and `]` for previous/next sprint.

**U12** — Toast notifications auto-dismiss but there's no progress indicator showing how long until dismissal. Add a shrinking progress bar.

---

## Accessibility (10 items)

**A1** — Sidebar navigation has no `role="navigation"` landmark. Screen readers can't identify it as the main nav. (The `<nav>` tag exists but the sidebar buttons lack `role="tablist"` semantics since they control tab panels.)

**A2** — Sprint board columns lack `role="list"` and draggable items lack `role="listitem"`. Drag-and-drop is mouse-only with no keyboard alternative.

**A3** — Color-only status indicators (heatmap, priority dots, connection status). No text alternative for colorblind users. Priority should show number, not just color.

**A4** — `NumInput` in Settings has no `aria-label`. Screen readers announce it as just "spinbutton" with no context.

**A5** — Modal dialogs (confirm, shortcuts) don't trap focus. Tab key can escape the dialog and interact with background elements.

**A6** — Toast notifications use `role="status"` but error toasts should use `role="alert"` for immediate announcement.

**A7** — Timer SVG circle progress has no text alternative. Screen readers see nothing for the visual progress indicator.

**A8** — Room voting interface has no keyboard navigation between cards. Users must click each card.

**A9** — `Select` component (custom dropdown) doesn't support arrow key navigation or type-ahead search. Standard `<select>` accessibility patterns are missing.

**A10** — Sprint board drag-and-drop has no keyboard alternative. Tasks can only be moved between columns via mouse drag. Add keyboard-accessible move buttons.

---

## Validation (8 items)

**V1** — `log_burn` doesn't validate that the task belongs to the sprint. Any task_id can be burned against any sprint_id.

**V2** — `create_webhook` URL validation doesn't check for non-HTTP schemes after the initial check. A URL like `http://evil.com/redirect?to=file:///etc/passwd` passes validation.

**V3** — `create_room` doesn't validate `room_type`. Only "estimation" is supported but any string is accepted.

**V4** — `add_team_member` doesn't validate the role. Any string is accepted as a team member role. Should validate against allowed roles.

**V5** — `create_team` doesn't validate team name length or characters. Empty names and extremely long names are accepted.

**V6** — `create_epic_group` doesn't validate name length. No max length check.

**V7** — `add_sprint_root_tasks` and `add_epic_group_tasks` don't validate that task_ids actually exist before inserting. Foreign key constraint will catch it but the error message is unhelpful.

**V8** — `set_recurrence` doesn't validate the `pattern` field. Only "daily", "weekly", "biweekly", "monthly" are handled in the processing loop, but any string is accepted and silently ignored.

---

## Authorization (6 items)

**Z1** — `add_team_member` has no authorization check. Any authenticated user can add members to any team. Should require team admin or root.

**Z2** — `remove_team_member` has no authorization check. Any user can remove any member from any team.

**Z3** — `add_team_root_tasks` / `remove_team_root_task` have no authorization. Any user can modify any team's task scope.

**Z4** — `delete_epic_group` has no ownership check. Any user can delete any epic group. Should check `created_by`.

**Z5** — `add_epic_group_tasks` / `remove_epic_group_task` have no ownership check. Any user can modify any epic group's tasks.

**Z6** — `snapshot_sprint` / `snapshot_epic_group` have no authorization. Any user can trigger snapshots for any sprint/epic. Should require sprint owner or root.

---

## Tests (12 items)

**T1** — No test for `tick()` auto-start behavior. When `auto_start_breaks` is true, the next session should start automatically after work completes.

**T2** — No test for midnight daily counter reset. The tick loop resets `daily_completed` at midnight but this is untested.

**T3** — No test for recurring task creation. The recurrence processing loop creates tasks but has no integration test.

**T4** — No test for WebSocket room state updates. The WS endpoint is untested.

**T5** — No test for sprint snapshot accuracy. Snapshot calculates done_points/done_hours but correctness is untested.

**T6** — No test for export CSV format correctness. CSV escaping and field ordering are untested.

**T7** — No test for team authorization (Z1-Z3). Team member/root operations have no auth tests.

**T8** — No test for epic group authorization (Z4-Z5). Epic operations have no auth tests.

**T9** — No frontend test for SSE reconnection behavior. The fallback polling logic is untested.

**T10** — No frontend test for keyboard shortcuts. Tab switching, search focus, refresh shortcut are untested.

**T11** — No frontend test for tree building with circular parent references. `buildTree` could infinite-loop if data has cycles.

**T12** — No test for attachment size limit enforcement. The 10MB limit is set but untested.

---

## Documentation (6 items)

**D1** — No API rate limiting documentation. Users don't know the limits (10 req/60s for auth, unknown for general endpoints).

**D2** — No webhook payload documentation. Webhook consumers don't know the event types or payload schemas.

**D3** — No deployment guide for production (reverse proxy, HTTPS, systemd service, backup strategy).

**D4** — No database migration strategy documented. The `migrate()` function uses `CREATE TABLE IF NOT EXISTS` but there's no versioned migration system for schema changes.

**D5** — No CLI (`poflow-cli`) usage documentation. The CLI exists but has no README or man page.

**D6** — OpenAPI spec is missing several endpoints: `room_ws`, `get_sprint_scope`, `get_team_scope`, `get_my_teams` are not in the `#[openapi]` paths list.

---

## i18n (4 items)

**I1** — Timer phase names ("Work", "Short Break", "Long Break") are hardcoded in English in `engine.rs` session_type field. These appear in history/export and aren't translatable.

**I2** — Error messages from the backend are all in English. Frontend displays them directly in toasts. Should have error codes that the frontend maps to localized strings.

**I3** — Date formatting uses ISO format everywhere. Should respect locale preferences (e.g., DD/MM/YYYY for Turkish locale).

**I4** — Keyboard shortcuts panel text is hardcoded in English. Should use i18n keys.

---

## Summary

| Category | Count |
|---|---|
| Bugs | 18 |
| Security | 10 |
| Features | 14 |
| Performance | 8 |
| Code Quality | 12 |
| UX | 12 |
| Accessibility | 10 |
| Validation | 8 |
| Authorization | 6 |
| Tests | 12 |
| Documentation | 6 |
| i18n | 4 |
| **Total** | **120** |

## Priority Order

1. **Security** S1-S10 (especially S1 HMAC, S2/S3 auth storage, S5 CORS)
2. **Bugs** B1-B18 (especially B1 lock contention, B4 skip counter, B5 HMAC, B11 dead SSE fallback)
3. **Authorization** Z1-Z6 (team/epic endpoints completely unprotected)
4. **Validation** V1-V8
5. **Features** F1-F14
6. **Performance** P1-P8
7. **Code Quality** Q1-Q12
8. **UX** U1-U12
9. **Accessibility** A1-A10
10. **Tests** T1-T12 (write alongside fixes)
11. **Documentation** D1-D6
12. **i18n** I1-I4
