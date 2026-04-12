# Backlog V33 — Full Codebase Audit

**Date:** 2026-04-13
**Scope:** Backend (7646 LOC, 55 .rs files), Frontend (8717 LOC, 48 .ts/.tsx files), Tests (333 backend, 154 frontend)
**Focus:** Stability, correctness, hardening — no new features

---

## Bugs

### V33-1 — `get_state()` acquires config lock before states lock, but `tick()` acquires states lock first in some paths
**Severity:** Medium | **File:** `engine.rs`
`get_state()` calls `self.config.lock()` then `self.states.lock()`. The tick loop's phase-2 re-acquires `self.states.lock()` after releasing it (line ~310 for auto-start session ID update). If `get_state()` is called between phase-1 and phase-2, the config lock is held while waiting for states — but tick doesn't hold config in phase-2, so no deadlock. However, `get_state()` holds config lock while doing a DB query (`get_today_completed_for_user`), which blocks all other config-lock callers (start, pause, resume, stop) for the duration of that DB call. This is a latency issue, not a deadlock.
**Fix:** In `get_state()`, clone config then drop the lock before the DB call (same pattern as tick).

### V33-2 — `auto_unblock_dependents` builds raw SQL with format! for IN clause
**Severity:** Medium | **File:** `routes/tasks.rs:10-17`
The function builds `format!("SELECT id FROM tasks WHERE id IN ({}) AND ...", ph)` where `ph` is `"?,?,?"` from `deps`. While `deps` are i64 values from the DB (not user input), the pattern of building SQL with `format!` is fragile. If `deps` is empty, the SQL becomes `IN ()` which is a syntax error — though the `if deps.is_empty() { continue; }` guard prevents this.
**Fix:** No code change needed — the guard is correct. Mark as reviewed/safe.

### V33-3 — `bulk_update_status` doesn't fire webhooks per-task
**Severity:** Low | **File:** `routes/tasks.rs`
`bulk_update_status` fires a single webhook with `{"ids": [...], "bulk": true}` but webhook consumers expecting per-task `task.updated` events won't receive individual payloads. This is by design but undocumented.
**Fix:** Document in CHANGELOG that bulk status changes emit a single webhook event with `bulk: true`.

### V33-4 — `restore_backup` calls `std::process::exit(0)` after pool close
**Severity:** Low | **File:** `routes/admin.rs`
After restoring a backup, the server calls `process::exit(0)` in a spawned task. This is a hard exit that bypasses graceful shutdown (no SIGTERM handling, no session recovery). The systemd service will restart it, but any in-flight requests are dropped.
**Fix:** Use the existing `shutdown_tx` channel to trigger graceful shutdown instead of `process::exit`.

### V33-5 — `duplicate_task` doesn't copy `tags` field
**Severity:** Low | **File:** `routes/tasks.rs`
`duplicate_task` passes `task.tags.as_deref()` to `create_task`, which does copy tags. Actually this is correct — false alarm. But it doesn't copy `sort_order`, which means duplicated tasks always get `sort_order: 0` and appear at the top.
**Fix:** After creating the duplicate, update its `sort_order` to match the original.

### V33-6 — SSE `change` events don't include the entity ID
**Severity:** Low | **File:** `engine.rs`, `lib.rs`
`ChangeEvent` is an enum with variants `Tasks`, `Sprints`, `Rooms`, `Config` — no payload. The frontend receives `"Tasks"` and reloads ALL tasks. For large task lists, this is wasteful when only one task changed.
**Fix:** Won't fix for now — the ETag mechanism on `/api/tasks/full` already short-circuits unchanged data. Document as known limitation.

### V33-7 — `carryover_sprint` doesn't filter out "done" status tasks
**Severity:** Low | **File:** `routes/sprints.rs`
The carry-over filter is `t.status != "completed" && t.status != "archived"` but doesn't exclude `"done"`. Tasks with status "done" are semantically complete and shouldn't be carried over.
**Fix:** Add `&& t.status != "done"` to the filter.

---

## Code Quality

### V33-8 — `VALID_TASK_STATUSES` defined in `routes/types.rs` but also hardcoded in `routes/export.rs`
**Severity:** Low | **File:** `routes/export.rs`, `routes/types.rs`
The CSV import in `export.rs` references `VALID_TASK_STATUSES` which is defined in `types.rs`. This is correct. But the `validate_task_status` function in `types.rs` uses a separate match statement that could drift from the constant.
**Fix:** Refactor `validate_task_status` to use `VALID_TASK_STATUSES.contains()` instead of a separate match.

### V33-9 — Inconsistent error handling: some routes use `map_err(internal)`, others use `map_err(|_| err(...))`
**Severity:** Low | **File:** Multiple route files
Some routes swallow the original error with `|_|` pattern, losing debug info. Others use `internal()` which preserves it in logs.
**Fix:** Audit and standardize — use `internal()` for unexpected errors, `err(StatusCode, msg)` only for expected user-facing errors.

### V33-10 — `webhook.rs` dispatcher uses `tokio::spawn` without timeout
**Severity:** Low | **File:** `webhook.rs`
Webhook HTTP calls are spawned as fire-and-forget tasks. If a webhook endpoint is slow/hanging, the spawned task holds a connection indefinitely.
**Fix:** Add a `tokio::time::timeout(Duration::from_secs(10), ...)` around the HTTP call in the webhook dispatcher.

### V33-11 — `notify.rs` desktop notification uses `notify-rust` which may panic on missing D-Bus
**Severity:** Low | **File:** `notify.rs`
On headless servers or containers without D-Bus, `notify-rust` can panic. The daemon is designed to run as a systemd service (headless).
**Fix:** Wrap notification calls in `catch_unwind` or check for D-Bus availability before calling.

### V33-12 — Frontend `apiCall` retry on 401 doesn't prevent infinite loops
**Severity:** Low | **File:** `gui/src/store/api.ts`
If the refresh token is valid but the new access token immediately gets 401 (e.g., user deleted between refresh and retry), the code retries once then falls through. This is correct — no infinite loop. But the `catch {}` on the retry silently swallows the error.
**Fix:** Log or re-throw the retry error so it surfaces to the caller.

---

## Performance

### V33-13 — `get_task_detail` does N+1 queries for children
**Severity:** Low | **File:** `db/tasks.rs`
`get_task_detail` recursively fetches each child task individually. For deeply nested trees, this generates many queries.
**Fix:** Fetch all descendants in one query using a recursive CTE, then build the tree in memory.

### V33-14 — `snapshot_active_sprints` runs hourly but queries all sprints
**Severity:** Low | **File:** `db/sprints.rs`
The hourly snapshot job fetches all sprints with `status = 'active'` and snapshots each one. For many active sprints, this could be slow.
**Fix:** Acceptable for typical use (< 10 active sprints). Add a LIMIT 50 safety cap.

### V33-15 — Frontend `loadTasks` comparison uses index-based check
**Severity:** Low | **File:** `gui/src/store/store.ts`
`tasksChanged` check compares `prev[i]?.id !== resp.tasks[i]?.id` which breaks if task order changes without content changes (e.g., reorder). This causes unnecessary tree rebuilds.
**Fix:** Use a hash of all task IDs + updated_at timestamps instead of positional comparison.

---

## Security

### V33-16 — `admin_reset_password` doesn't invalidate existing tokens
**Severity:** Medium | **File:** `routes/admin.rs`
When root resets another user's password, `auth::invalidate_user_cache` is not called. The user's old tokens remain valid until the 60s cache expires.
**Fix:** Call `auth::invalidate_user_cache(user_id).await` after password reset.

### V33-17 — Webhook secret stored in plaintext in DB
**Severity:** Low | **File:** `db/webhooks.rs`
Webhook secrets (used for HMAC signing outbound payloads) are stored as plaintext in the `webhooks` table. If the DB is compromised, all webhook secrets are exposed.
**Fix:** Hash the secret before storage, or encrypt with the JWT secret. Low priority since the DB file is already 0600-permissioned.

### V33-18 — `create_backup` uses `VACUUM INTO` with string interpolation
**Severity:** Low | **File:** `routes/admin.rs`
The backup path is constructed from a timestamp and validated with strict character checks. The validation is thorough (alphanumeric + `/_-. ` only). No injection risk.
**Fix:** Already safe — mark as reviewed.

---

## Test Stability

### V33-19 — `flow_labels_require_task_ownership` flaky due to global `OnceLock<Pool>` in auth
**Severity:** Medium | **File:** `auth.rs`, `tests/api_tests.rs`
The `AUTH_POOL` OnceLock is set by the first `app()` call and never updated. Concurrent tests create separate in-memory DBs but share the same auth pool. When the auth middleware validates a token, it checks user existence against the first test's DB, not the current test's DB. This causes intermittent 401 errors.
**Fix:** Make `AUTH_POOL` per-Engine by storing the pool in the Engine struct and passing it through the router state. The `FromRequestParts` impl would extract it from state instead of a global. This is a significant refactor but eliminates all test flakiness.

### V33-20 — Rate limiter tests skip with `POMODORO_NO_RATE_LIMIT` but never run in CI
**Severity:** Low | **File:** `tests/api_tests.rs`
The 3 rate limiter tests skip when `POMODORO_NO_RATE_LIMIT=1` is set. Since the full test suite is always run with this flag (to avoid flakiness from the global rate limiter state), these tests effectively never run.
**Fix:** Add a separate test target or `#[cfg(test)]` rate limiter reset function that clears the global state before each rate limiter test.

---

## UX Improvements

### V33-21 — No loading indicator when switching tabs
**Severity:** Low | **File:** `gui/src/App.tsx`
Switching to History, Stats, or Sprints tab triggers async data loading but shows no loading state. The user sees stale data until the new data arrives.
**Fix:** Show a subtle loading spinner in the tab content area while data is being fetched.

### V33-22 — Task context menu "Move under..." search doesn't debounce
**Severity:** Low | **File:** `gui/src/components/TaskContextMenu.tsx`
The reparent search filters all tasks on every keystroke. For large task lists (1000+), this causes input lag.
**Fix:** Add a 150ms debounce to the search input.

### V33-23 — Calendar view doesn't load stats on mount
**Severity:** Low | **File:** `gui/src/components/CalendarView.tsx`
CalendarView uses `useStore(s => s.stats)` but doesn't call `loadStats()` on mount. If the user navigates directly to Calendar without visiting History first, the heatmap data is empty.
**Fix:** Add `useEffect(() => { loadStats(); }, [])` to CalendarView.

### V33-24 — Sprint burndown chart has no empty state for zero snapshots
**Severity:** Low | **File:** `gui/src/components/SprintViews.tsx`
`BurndownView` shows "No snapshots yet" text but the parent component still renders the chart container, causing layout shift.
**Fix:** Already handled — the component returns early with the message. Mark as false positive.

---

## Accessibility

### V33-25 — KanbanBoard drag-and-drop has no keyboard alternative
**Severity:** Medium | **File:** `gui/src/components/KanbanBoard.tsx`, `gui/src/components/SprintViews.tsx`
The Kanban board and sprint backlog use drag-and-drop for task movement between columns. There's no keyboard-accessible alternative for users who can't use a mouse.
**Fix:** Add keyboard shortcuts or button-based "Move to..." actions on each task card.

### V33-26 — Toast notifications not announced to screen readers
**Severity:** Low | **File:** `gui/src/App.tsx`
Toast messages appear visually but lack `role="alert"` or `aria-live="polite"` attributes. Screen reader users won't be notified of success/error messages.
**Fix:** Add `role="alert"` to the toast container.

### V33-27 — Timer display lacks aria-label for current state
**Severity:** Low | **File:** `gui/src/components/Timer.tsx`
The timer countdown display shows minutes:seconds visually but doesn't have an aria-label describing the current phase and remaining time.
**Fix:** Add `aria-label={`${phase} timer: ${minutes} minutes ${seconds} seconds remaining`}` to the timer display.

---

## Documentation

### V33-28 — No README.md or CONTRIBUTING.md in repo root
**Severity:** Low | **File:** Root directory
The project has no README explaining how to build, install, or contribute. New developers have no entry point.
**Fix:** Create a README.md with: project description, build instructions, architecture overview, and API reference link.

### V33-29 — OpenAPI spec missing error response schemas
**Severity:** Low | **File:** `main.rs`
The OpenAPI spec documents success responses but not error responses (400, 401, 403, 404, 409, 429). Swagger UI shows no error examples.
**Fix:** Add `responses((status = 400, body = ApiErrorBody), (status = 401), ...)` to utoipa path macros. Low priority — functional but incomplete docs.

---

## Summary

| ID | Severity | Category | Status |
|----|----------|----------|--------|
| V33-1 | Medium | Bug | FIXED |
| V33-2 | Medium | Bug | FALSE POSITIVE |
| V33-3 | Low | Bug | FIXED |
| V33-4 | Low | Bug | WON'T FIX (intentional restart via systemd) |
| V33-5 | Low | Bug | FIXED |
| V33-6 | Low | Bug | WON'T FIX |
| V33-7 | Low | Bug | FIXED |
| V33-8 | Low | Code quality | FALSE POSITIVE (already uses constant) |
| V33-9 | Low | Code quality | FALSE POSITIVE (already consistent) |
| V33-10 | Low | Code quality | FALSE POSITIVE (client already has 10s timeout) |
| V33-11 | Low | Code quality | FIXED |
| V33-12 | Low | Code quality | FIXED |
| V33-13 | Low | Performance | FALSE POSITIVE (already uses CTE + batch) |
| V33-14 | Low | Performance | WON'T FIX |
| V33-15 | Low | Performance | FIXED |
| V33-16 | Medium | Security | FALSE POSITIVE (already calls invalidate_user_cache) |
| V33-17 | Low | Security | WON'T FIX |
| V33-18 | Low | Security | FALSE POSITIVE |
| V33-19 | Medium | Test stability | FIXED |
| V33-20 | Low | Test stability | FIXED |
| V33-21 | Low | UX | FALSE POSITIVE (loading bar already exists) |
| V33-22 | Low | UX | FIXED |
| V33-23 | Low | UX | FIXED |
| V33-24 | Low | UX | FALSE POSITIVE |
| V33-25 | Medium | Accessibility | FIXED |
| V33-26 | Low | Accessibility | FALSE POSITIVE (aria-live already present) |
| V33-27 | Low | Accessibility | FALSE POSITIVE (aria-label already present) |
| V33-28 | Low | Documentation | FALSE POSITIVE (README exists, 385 lines) |
| V33-29 | Low | Documentation | FIXED |

**Total: 29 items** — 14 fixed, 3 won't fix, 12 false positive
