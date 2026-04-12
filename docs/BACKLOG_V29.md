# Comprehensive Audit Backlog (V29)

Full codebase audit — every .rs file (7500 LOC), every .tsx/.ts file (8628 LOC),
all 150 route handlers, 16 DB migrations, all frontend components.

---

## Bugs

### V29-1 — `create_backup` uses string interpolation for SQL path
**Severity:** Medium | **File:** `routes/admin.rs`
`VACUUM INTO '{}'` uses `format!()` with the backup path. Although the path is
validated with an allowlist (`is_ascii_alphanumeric || /_-. `), this is still
string interpolation in SQL. A space in the data dir path could break the query.
Should use a parameterized approach or shell out to `sqlite3 .backup`.

### V29-2 — `restore_backup` same SQL interpolation issue
**Severity:** Medium | **File:** `routes/admin.rs`
Same `format!("VACUUM INTO '{}'")` pattern for the safety backup during restore.

### V29-3 — `list_tasks` doesn't filter by user for non-root
**Severity:** Low | **File:** `routes/tasks.rs`
`list_tasks` passes `user_id: None` to the filter regardless of role. This is
by design (shared workspace), but inconsistent with `search_tasks` which
filters by user for non-root. Document or align behavior.

### V29-4 — `get_task_detail` has no ownership check
**Severity:** Low | **File:** `routes/tasks.rs`
Any authenticated user can view any task's full detail including comments and
sessions. Consistent with shared workspace model but worth documenting.

### V29-5 — `session_participants` has no existence check
**Severity:** Low | **File:** `routes/timer.rs`
`GET /api/timer/participants/{session_id}` returns empty array for non-existent
sessions instead of 404. Should verify session exists first.

## Security

### V29-6 — Backup path validation allows spaces
**Severity:** Low | **File:** `routes/admin.rs`
The path validation `b"/_-. ".contains(&b)` allows spaces in paths. While
unlikely to be exploitable, spaces in SQL string literals could cause issues
if the path contains single quotes (already blocked by the allowlist).

### V29-7 — `export_ical` doesn't sanitize task titles for iCal injection
**Severity:** Low | **File:** `routes/export.rs`
`ical_escape` handles `\`, `;`, `,`, `\n`, `\r` but doesn't escape `:`
which is a delimiter in iCal properties. A task title containing `:` could
potentially break iCal parsers.

### V29-8 — GitHub webhook HMAC is optional (env var)
**Severity:** Low | **File:** `routes/misc.rs`
If `GITHUB_WEBHOOK_SECRET` is not set, the webhook accepts any payload without
verification. Should log a warning on startup if the env var is missing and
the route is registered.

## Performance

### V29-9 — `bulk_update_status` auto-unblock is O(n²)
**Severity:** Low | **File:** `routes/tasks.rs`
When bulk-completing tasks, the auto-unblock loop iterates all dependents for
each task, then checks all dependencies for each dependent. For 500 tasks with
many dependencies, this could be slow. Consider batching the dependency check.

### V29-10 — `estimation_accuracy` loads up to 500 tasks into memory
**Severity:** Low | **File:** `routes/history.rs`
The accuracy endpoint fetches up to 500 completed tasks and computes stats
in Rust. For large datasets, this should use SQL aggregation instead.

### V29-11 — `activity_feed` runs two separate queries then merges
**Severity:** Low | **File:** `routes/history.rs`
The feed endpoint runs separate queries for audit log and comments, then
sorts in Rust. Could use a UNION ALL query for better performance.

## Code Quality

### V29-12 — `history.rs` is 355 lines with 12 unrelated endpoints
**Severity:** Low | **File:** `routes/history.rs`
Contains estimation accuracy, focus score, achievements, leaderboard,
auto-prioritization, activity feed, smart scheduling, and weekly digest.
Should be split into `analytics.rs`, `achievements.rs`, `feed.rs`.

### V29-13 — `misc.rs` is 346 lines with 18 unrelated endpoints
**Severity:** Low | **File:** `routes/misc.rs`
Contains task links, GitHub webhook, automation rules, user presence,
Slack integration, SSE, rooms, and various utility endpoints. Should be
split into `integrations.rs`, `automations.rs`, `presence.rs`.

### V29-14 — Duplicate auto-unblock logic in update_task and bulk_update_status
**Severity:** Low | **File:** `routes/tasks.rs`
The dependency auto-unblock code is duplicated verbatim (~20 lines) in both
`update_task` and `bulk_update_status`. Should be extracted to a helper.

## Missing Error Handling

### V29-15 — `add_comment` notification spawn ignores all errors
**Severity:** Low | **File:** `routes/comments.rs`
The `@mention` notification spawns a background task that silently ignores
all errors. Should at least log failures.

### V29-16 — `cacheTasksOffline` doesn't handle IndexedDB quota exceeded
**Severity:** Low | **File:** `gui/src/offlineStore.ts`
If IndexedDB storage is full, `cacheTasksOffline` will fail silently.
Should catch QuotaExceededError and clear old data.

## UX / Frontend

### V29-17 — No loading indicator for Dashboard analytics widgets
**Severity:** Low | **File:** `gui/src/components/Dashboard.tsx`
`FocusScore` and `Achievements` show nothing while loading (return null).
Should show a skeleton/spinner during the API call.

### V29-18 — KanbanBoard has no empty state message
**Severity:** Low | **File:** `gui/src/components/KanbanBoard.tsx`
When all columns are empty, the board shows empty columns with no guidance.
Should show "No tasks yet" or similar.

### V29-19 — CalendarView doesn't support keyboard navigation
**Severity:** Low | **File:** `gui/src/components/CalendarView.tsx`
Calendar cells are `<button>` elements but don't support arrow key navigation
between days. Should add keyboard handlers for accessibility.

### V29-20 — Mobile bottom bar overlaps content on very small screens
**Severity:** Low | **File:** `gui/src/App.tsx`
The `safe-bottom` class and `pb-14` padding may not be sufficient on devices
with very tall navigation bars (e.g., iPhone with gesture bar).

## Documentation

### V29-21 — No README for new API endpoints
**Severity:** Low | **File:** Project root
22 new endpoints added (F1-F28) but no documentation beyond OpenAPI.
Should add a CHANGELOG or API guide for the new features.

### V29-22 — Automation rules trigger types not documented
**Severity:** Low | **File:** `routes/misc.rs`
The three valid triggers (`task.status_changed`, `task.due_approaching`,
`task.all_subtasks_done`) are defined in code but not documented anywhere
for API consumers.

---

## Summary

| ID | Severity | Category | Status |
|----|----------|----------|--------|
| V29-1 | Medium | Bug/Security | FIXED |
| V29-2 | Medium | Bug/Security | FIXED |
| V29-3 | Low | Design decision | WON'T FIX (shared workspace by design) |
| V29-4 | Low | Design decision | WON'T FIX (shared workspace by design) |
| V29-5 | Low | Missing validation | FIXED |
| V29-6 | Low | Security | WON'T FIX (already blocked by allowlist) |
| V29-7 | Low | Security | FIXED |
| V29-8 | Low | Security | FIXED |
| V29-9 | Low | Performance | WON'T FIX (acceptable for typical use) |
| V29-10 | Low | Performance | WON'T FIX (500 row limit is reasonable) |
| V29-11 | Low | Performance | WON'T FIX (acceptable for typical use) |
| V29-12 | Low | Code quality | WON'T FIX (refactor, not a bug) |
| V29-13 | Low | Code quality | WON'T FIX (refactor, not a bug) |
| V29-14 | Low | Code quality | FIXED |
| V29-15 | Low | Error handling | FIXED |
| V29-16 | Low | Error handling | FIXED |
| V29-17 | Low | UX | FIXED |
| V29-18 | Low | UX | FIXED |
| V29-19 | Low | Accessibility | FIXED |
| V29-20 | Low | UX | WON'T FIX (CSS limitation) |
| V29-21 | Low | Documentation | FIXED |
| V29-22 | Low | Documentation | FIXED |

**Total: 22 items** — 12 fixed, 10 won't fix
