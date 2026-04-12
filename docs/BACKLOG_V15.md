# BACKLOG v15 — Fresh Codebase Audit

Audit date: 2026-04-12
Codebase: 6300+ LOC backend (55 .rs), 9000+ LOC frontend (53 .ts/.tsx)
Tests: 275 backend, 154 frontend

---

## Confirmed Bugs (15)

- [x] **B1.** `TaskLabelPicker` infinite API loop — `useEffect(load, [taskId])` where `load` is recreated every render, causing infinite calls. Same bug in `Recurrence.tsx`. Fix: wrap `load` in `useCallback`.
- [x] **B2.** `get_due_tasks` returns soft-deleted tasks — missing `AND deleted_at IS NULL` in WHERE clause. Deleted tasks with due dates still trigger notifications.
- [x] **B3.** `snapshot_sprint` counts soft-deleted tasks — aggregate query joins `sprint_tasks → tasks` without `WHERE t.deleted_at IS NULL`. Burndown totals include deleted tasks.
- [x] **B4.** `snapshot_epic_group` counts soft-deleted tasks — same issue as B3 for epic group snapshots.
- [x] **B5.** `teams.rs add_team_root_tasks` doesn't check `deleted_at IS NULL` — teams can reference soft-deleted tasks. `epics.rs` correctly checks this.
- [x] **B6.** `SprintParts.tsx BoardView` — `Column` useCallback has `changeStatus` in deps but `changeStatus` is a plain const that changes every render, making memoization useless. Causes unnecessary re-renders of all board columns.
- [x] **B7.** `Dashboard.tsx SprintProgress` — `pct` computed with `board!.done.length` non-null assertion executes before the `board && total > 0` guard. Will crash if `board` is null.
- [x] **B8.** `delete_user` doesn't clean up `notifications`, `notification_prefs`, `task_watchers`, `user_configs`, `team_members` tables — orphaned rows after user deletion.
- [x] **B9.** `add_sprint_tasks` loop without transaction — inserts tasks one-by-one. If one fails mid-way, partial inserts remain with no rollback.
- [x] **B10.** `delete_user` has no transaction wrapping — multiple DELETE/UPDATE statements. Process crash mid-way leaves inconsistent state.
- [x] **B11.** `get_active_webhooks` SQL LIKE injection — `format!("%{}%", event)` doesn't escape `%` or `_` wildcards in event names, matching unintended patterns.
- [x] **B12.** `check_fts5` OnceLock race — if called before `migrate()` sets the real value, it incorrectly reports FTS5 as available. `OnceLock::set()` silently fails if already initialized.
- [x] **B13.** Auto-archive background task doesn't emit `ChangeEvent::Tasks` — SSE clients won't see archived tasks until next manual refresh.
- [x] **B14.** `useRoomWebSocket` reconnect timer leak — `setTimeout(tryConnect, delay)` in `onclose` is never cleared on unmount. Same issue in `useSseConnection`.
- [x] **B15.** `Select.tsx` Space key conflict — pressing Space in the filter input triggers option selection instead of typing a space character.

## Security (8)

- [x] **S1.** `add_assignee` has no ownership check — any authenticated user can assign anyone to any task. Only `remove_assignee` checks ownership.
- [x] **S2.** `get_room_state` has no membership check — any authenticated user can view any room's full state including votes, even though `list_rooms` restricts to own rooms.
- [x] **S3.** Audit log exposed to all users — `audit.rs` has no authorization check. Any authenticated user can read the full audit log including admin operations.
- [x] **S4.** Webhook SSRF IPv6 bypass — `is_private_ip` only checks `is_loopback()` and `is_unspecified()` for IPv6. Missing: link-local (`fe80::/10`), unique local (`fc00::/7`), IPv4-mapped (`::ffff:127.0.0.1`).
- [x] **S5.** Webhook SSRF DNS rebinding — `is_safe_url()` resolves DNS then reqwest re-resolves. Attacker can use DNS rebinding (first resolution public, second 127.0.0.1) to bypass the check.
- [x] **S6.** JWT secret fallback uses predictable entropy — when `/dev/urandom` unavailable, falls back to `SHA256(timestamp + pid)`. Should refuse to start without proper entropy.
- [x] **S7.** `dangerouslySetInnerHTML` for FTS search results — `TaskList.tsx` renders `r.title` and `r.snippet` from server without sanitization. XSS vector if server doesn't sanitize.
- [x] **S8.** `admin.rs create_backup` uses string-formatted SQL — `format!("VACUUM INTO '{}'", path_str)` is a SQL injection vector. Should validate path more strictly.

## Business Logic (10)

- [x] **BL1.** `delete_comment` has no ownership check in DB layer — any user could delete any comment if route handler doesn't verify. Add `user_id` guard to DB function.
- [x] **BL2.** `delete_webhook`, `remove_assignee`, `remove_dependency`, `remove_sprint_task`, `leave_room` all silently succeed on non-existent records — should check `rows_affected()` and return 404.
- [x] **BL3.** `export.rs import_tasks_csv` and `import_tasks_json` have no transaction — partial imports leave orphaned tasks on later failures.
- [x] **BL4.** `token_blocklist` has no cleanup — expired tokens accumulate forever. Add periodic cleanup of rows where `expires_at < now`.
- [x] **BL5.** `list_attachments` has no access control while `download_attachment` does — inconsistent. Either both should check or neither.
- [x] **BL6.** `create_task` has no validation that `parent_id` exists or belongs to the user — can create orphaned subtasks.
- [x] **BL7.** `duplicate_task` has no ownership check — any user can duplicate any other user's task.
- [x] **BL8.** `rooms.rs accept_estimate` auto-advance uses `.next()` without deterministic ordering — "next unestimated task" is arbitrary. Should order by sort_order or ID.
- [x] **BL9.** (won't-fix: intentional) `recover_interrupted` marks ALL running sessions as interrupted with no user_id filter — multi-user scenario could interrupt other users' sessions on restart.
- [x] **BL10.** (won't-fix: best-effort) `tasks.rs update_task` auto-unblock has N+1 query pattern (loop dependents → loop deps → fetch each task) with no transaction wrapping. Partial unblocking on failure.

## UX Improvements (8)

- [x] **UX1.** `EpicBurndown` — no confirmation on delete. Clicking `×` immediately destroys epic group data.
- [x] **UX2.** `TeamManager` — no confirmation on team delete.
- [x] **UX3.** `Labels.tsx` — no confirmation on label delete.
- [x] **UX4.** `AuditLog` — no pagination. Loads all entries at once despite API supporting `?page=&per_page=`.
- [x] **UX5.** `Dependencies.tsx` — select dropdown only shows first 20 tasks with no search/filter. Unusable for large task lists.
- [x] **UX6.** `TaskNode.tsx` — uses native `alert()` for error display instead of toast system. Blocks UI.
- [x] **UX7.** `AuthScreen.tsx` — password placeholder says "min 6 chars" but no client-side validation enforces this before submit.
- [x] **UX8.** `TaskList.tsx` — paste handler for bulk task creation has no limit. Pasting 1000 lines fires 1000 sequential API calls.

## Accessibility (8)

- [x] **A1.** `EpicBurndown` — delete control is a `<span>` not `<button>`, not keyboard-focusable, no ARIA role. Multiple buttons lack `aria-label`.
- [x] **A2.** `AuditLog` — `role="row"` divs have no `role="cell"` on data spans. Screen readers can't parse table structure.
- [x] **A3.** `Select.tsx` — missing `aria-activedescendant` and `id` on options. Keyboard focus tracking is visual only.
- [x] **A4.** `ErrorBoundary` — no `role="alert"` or `aria-live`. Screen readers won't announce crashes.
- [x] **A5.** `Labels.tsx` — color input has no associated label or `aria-label`.
- [x] **A6.** `SprintParts.tsx BoardView` — board items don't have `role="listitem"` despite columns having `role="list"`.
- [x] **A7.** App.tsx connection status indicator — plain `div` with only `title`. Should use `role="status"` with `aria-live` for screen reader announcements.
- [x] **A8.** Color contrast — many elements use `text-white/20` and `text-white/30` which likely fail WCAG AA contrast requirements.

## i18n Gaps (5)

- [x] **I1.** `ErrorBoundary` — "Something went wrong" and "Reload" hardcoded English.
- [x] **I2.** `Recurrence.tsx` — "Add recurrence", "edit", "remove", "Save", "Cancel" hardcoded despite locale keys existing.
- [x] **I3.** `EpicBurndown.tsx` — all strings hardcoded English ("Epic Burndown", "Root tasks in group", "Snapshot now", etc.).
- [x] **I4.** `TeamManager.tsx` — "Teams", "Members", "Delete team", "No teams yet" hardcoded.
- [x] **I5.** `AuditLog.tsx` — filter options "All", "Tasks", "Users", "Sprints", "Rooms" hardcoded.

## Performance (4)

- [x] **P1.** `TASK_SELECT` correlated subquery — `(SELECT COUNT(*) FROM task_attachments WHERE task_id = t.id)` runs per row. Expensive for list queries. Use LEFT JOIN or compute separately.
- [x] **P2.** (won't-fix: too invasive) Engine lock contention — `states` HashMap behind single `tokio::sync::Mutex`. Every tick locks for all users. Consider `DashMap` or per-user locks.
- [x] **P3.** (done via P4) `get_velocity` query has no index support — JOINs sprints → burn_log → sprint_tasks → tasks with GROUP BY. Add composite index on `(sprint_id, cancelled)`.
- [x] **P4.** Missing DB indexes — no index on `notifications(user_id, read)`, no index on `task_watchers(user_id)`.

## Infrastructure (5)

- [x] **INF1.** No SIGTERM handling — graceful shutdown only handles SIGINT (ctrl_c). Systemd sends SIGTERM. Add `tokio::signal::unix::signal(SignalKind::terminate())`.
- [x] **INF2.** Request ID collisions — generated from `subsec_nanos()` only (8 hex chars). Two requests in same nanosecond get identical IDs. Use `AtomicU64` counter.
- [x] **INF3.** Missing `Content-Security-Policy` header — security headers include X-Content-Type-Options, X-Frame-Options, Referrer-Policy but no CSP.
- [x] **INF4.** (won't-fix: breaks existing data) `now_str()` timestamps have no timezone indicator — `2026-04-12T11:28:39.018` is ambiguous UTC vs local. Add `Z` suffix.
- [x] **INF5.** Migration errors silently swallowed — all ALTER TABLE / CREATE TABLE use `.ok()`. Genuine errors (disk full, corruption) are hidden. Should log warnings.

## Code Quality (5)

- [x] **CQ1.** `watchers.rs` — missing all `#[utoipa::path]` annotations. Endpoints won't appear in OpenAPI/Swagger docs.
- [x] **CQ2.** (won't-fix: cosmetic) Sprint-related routes misplaced in `epics.rs` — `get_sprint_root_tasks`, `get_sprint_scope`, `snapshot_sprint`, `get_sprint_board` belong in sprints module.
- [x] **CQ3.** (won't-fix: large refactor) `TaskNode.tsx` excessive prop drilling — 8+ props drilled through every recursive node. Should use React context.
- [x] **CQ4.** (won't-fix: needs crypto dep) Webhook secret "encryption" is XOR obfuscation — trivially reversible. Should use AES-GCM or similar authenticated encryption.
- [x] **CQ5.** `i18n.ts` — no fallback for missing translation keys. Accessing a missing key returns `undefined` with no warning. Should fall back to English.

---

**Total: 68 items** (15 bugs, 8 security, 10 business logic, 8 UX, 8 accessibility, 5 i18n, 4 performance, 5 infrastructure, 5 code quality)
