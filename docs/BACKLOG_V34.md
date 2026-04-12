# Backlog V34 — Full Codebase Audit (2026-04-13)

Scope: Stability, correctness, security, performance, UX, accessibility, code quality.
No new features.

---

## V34-1 [Medium / Bug] `notify_due_task` not wrapped in `catch_unwind`
**File:** `notify.rs:38-47`
`notify_due_task` calls `notify_rust::Notification::show()` without `catch_unwind`, unlike `notify_session_complete` which was fixed in V33-11. Same D-Bus panic risk on headless servers.

## V34-2 [Medium / Security] `create_backup` uses format string with user-influenced path
**File:** `routes/admin.rs:68`
`VACUUM INTO '{}'` uses `format!` with the backup path. Although the path is server-generated (timestamp-based) and validated for dangerous chars, this is still a SQL injection vector if the validation is ever relaxed. Should use a parameterized approach or at minimum document the risk.

## V34-3 [Medium / Bug] `restore_backup` safety backup has same format-string risk
**File:** `routes/admin.rs:128`
Same `VACUUM INTO '{}'` pattern with `safety_str`. Same mitigation needed.

## V34-4 [Medium / Performance] `auto_unblock_dependents` does N+1 queries per dependent
**File:** `routes/tasks.rs:8-20`
For each dependent task, it fetches the task, then its dependencies, then runs a batch query. If a completed task has many dependents, this is O(n) DB round-trips. Should batch-fetch all dependent tasks and their dependency statuses in fewer queries.

## V34-5 [Low / Bug] `whoami::hostname()` deprecated warning in GUI
**File:** `gui/src-tauri/src/lib.rs:151`
Build warning: `use of deprecated function whoami::hostname`. Should migrate to `whoami::fallible::hostname()`.

## V34-6 [Low / Bug] `beforeBundleCommand` stderr redirect is a workaround
**File:** `gui/src-tauri/tauri.conf.json:11`
`cargo build --release -p pomodoro-daemon 2>&1` works around a Tauri bug where stderr output causes "failed with exit code 0". Should track upstream fix and revert.

## V34-7 [Low / Security] Webhook private IP check doesn't cover 172.16-31 range completely
**File:** `routes/webhooks.rs` (create_webhook)
The `create_webhook` handler checks `host.starts_with("172.16.")` through `"172.31."` individually (16 prefixes). The `is_private_ip` function in `webhook.rs` correctly uses `v4.is_private()` which covers the full range. The route-level check is redundant but incomplete — it misses `172.16.0.0/12` as a CIDR. Not exploitable since `webhook.rs:dispatch` does the real check, but the route check gives false confidence.

## V34-8 [Low / Bug] `edit_comment` 15-minute window uses server time, not creation time
**File:** `routes/comments.rs:69`
The edit window compares `Utc::now()` against `created_at`. If the server clock drifts or the client is in a different timezone, the window may be shorter or longer than expected. Minor but worth documenting.

## V34-9 [Low / Code Quality] `update_webhook` builds SQL dynamically with string concatenation
**File:** `routes/webhooks.rs` (update_webhook)
Builds `UPDATE webhooks SET url = ?, events = ?` dynamically based on which fields are present. The values are bound as parameters (safe), but the column names are hardcoded strings (also safe). However, the pattern is fragile — adding a new field requires careful ordering of binds. Consider using a query builder or always updating all fields.

## V34-10 [Low / Bug] `leaderboard` query uses `date('now', ?)` with string interpolation
**File:** `routes/misc.rs` (leaderboard)
`format!("-{} days", days)` is passed as a bind parameter to SQLite's `date()` function. The `days` value comes from a parsed query parameter clamped to 7/30/365, so it's safe. But the pattern is unusual — normally you'd compute the cutoff date in Rust and bind it directly.

## V34-11 [Low / Performance] `get_tasks_full` ETag computation queries 7 aggregates in one query
**File:** `routes/misc.rs` (get_tasks_full)
The ETag query `SELECT COALESCE((SELECT MAX(...)), ''), (SELECT COUNT(*) FROM tasks...), ...` runs 7 subqueries. On large datasets this could be slow. Consider caching the ETag or using a simpler change-detection mechanism (e.g., a global version counter).

## V34-12 [Low / UX] CalendarView `useEffect` missing `loadStats` in dependency array
**File:** `gui/src/components/CalendarView.tsx:12`
`useEffect(() => { loadStats(); }, [])` — ESLint would flag missing `loadStats` dependency. Functionally correct (loadStats is stable from zustand) but should add to deps or use `// eslint-disable-next-line` comment.

## V34-13 [Low / Code Quality] `RateLimiter` struct and `reset()` are fully `pub` but only needed for tests
**File:** `routes/mod.rs:13,56`
`RateLimiter` and `reset()` were made `pub` to allow integration test access. Consider using `#[cfg(test)]` or `#[doc(hidden)]` to signal these are not part of the public API.

## V34-14 [Low / Bug] `focus_score` streak calculation allows today to be missing but doesn't handle it consistently
**File:** `routes/misc.rs` (focus_score)
The streak loop breaks if `check_date < today` and the date is missing, but if today itself is missing, it also breaks. This means if you haven't done any sessions today, your streak is 0 even if yesterday was active. The `check_achievements` function has the same logic. Should allow today to be missing (day not over yet) and start counting from yesterday.

## V34-15 [Low / Accessibility] KanbanBoard columns lack `role="list"` for screen readers
**File:** `gui/src/components/KanbanBoard.tsx`
Cards have `role="listitem"` but the parent column container doesn't have `role="list"`. Screen readers need the parent role to announce the list context.

## V34-16 [Low / UX] `TaskContextMenu` `isDescendant` helper is O(n²) for deep trees
**File:** `gui/src/components/TaskContextMenu.tsx:191`
The reparent search filters with `isDescendant(c.id, t.id, allTasks)` which walks the tree for each candidate. For large task lists with deep nesting, this could cause UI jank. Consider pre-computing the descendant set once.

## V34-17 [Low / Code Quality] Multiple `serde_json::json!` responses should use typed structs
**File:** Various route handlers (misc.rs, history.rs, sprints.rs)
Many endpoints return `ApiResult<serde_json::Value>` with inline `json!` macros. This bypasses compile-time type checking and makes the API contract implicit. Key offenders: `estimation_accuracy`, `focus_score`, `schedule_suggestions`, `weekly_digest`, `sprint_retro_report`, `compare_sprints`.

## V34-18 [Low / Bug] `import_tasks_csv` doesn't validate `estimated_hours` or `remaining_points`
**File:** `routes/export.rs` (import_tasks_csv)
CSV import sets `estimated_hours` and `remaining_points` to 0.0 hardcoded. If the CSV has these columns, they're ignored. Should parse them if present.

## V34-19 [Low / Security] SSE ticket uses `/dev/urandom` with fallback to hash-based entropy
**File:** `routes/misc.rs` (create_sse_ticket)
The fallback uses `Sha256(timestamp + user_id + counter)` which is predictable if an attacker knows the server start time. The JWT secret generation in `auth.rs` has the same pattern but logs a security warning. The SSE ticket should also log a warning on fallback.

## V34-20 [Low / UX] `AuthScreen` doesn't show password requirements until validation fails
**File:** `gui/src/components/AuthScreen.tsx`
Users only learn about the 8-char, uppercase, digit requirements after submitting. Should show requirements inline below the password field.

## V34-21 [Low / Performance] `activity_feed` fetches audit + comments separately, then sorts in memory
**File:** `routes/misc.rs` (activity_feed)
Two separate queries with `LIMIT ?` each, then merged and sorted. If the user requests 50 items, we fetch up to 100 rows total. Could use a UNION ALL query with a single ORDER BY and LIMIT.

## V34-22 [Low / Code Quality] `user_cache` in auth.rs is global, not per-router
**File:** `auth.rs:12-17`
The `USER_CACHE` is a global `OnceLock<RwLock<HashMap>>`. While the `FromRequestParts` now uses the per-router pool for DB queries (V33-19 fix), the cache itself is still shared across test routers. This means test A can cache user_id=1 and test B gets a false cache hit. The cache says "valid" so it skips the DB check — which is a false positive (accepts the token) rather than a false negative. Not a correctness issue in production (single router), but contributes to residual test flakiness.

## V34-23 [Low / Documentation] CHANGELOG v2.0.1 entry is sparse
**File:** `docs/CHANGELOG.md`
The v2.0.1 entry only mentions bulk webhook behavior and sprint carry-over. Should include all V33 fixes (auth per-router pool, get_state lock fix, keyboard nav, etc.).

## V34-24 [Low / Bug] `Sidebar` theme sync `useEffect` has stale closure over `theme`
**File:** `gui/src/App.tsx:42-45`
`useEffect` depends on `config?.theme` but reads `theme` from local state without including it in the dependency array. If `theme` changes from another source, the comparison `config.theme !== theme` may use a stale value.

## V34-25 [Low / UX] Mobile bottom nav shows 8 tabs — too many for small screens
**File:** `gui/src/App.tsx:355-367`
The mobile nav filters to 8 tabs (`timer, tasks, kanban, dashboard, sprints, rooms, history, settings`). On a 320px screen, each tab gets ~40px which is cramped. Should reduce to 5 core tabs with a "more" overflow menu.

---

## Summary

| ID | Severity | Category | Status |
|----|----------|----------|--------|
| V34-1 | Medium | Bug | |
| V34-2 | Medium | Security | |
| V34-3 | Medium | Security | |
| V34-4 | Medium | Performance | |
| V34-5 | Low | Bug | |
| V34-6 | Low | Bug | |
| V34-7 | Low | Security | |
| V34-8 | Low | Bug | |
| V34-9 | Low | Code quality | |
| V34-10 | Low | Bug | |
| V34-11 | Low | Performance | |
| V34-12 | Low | UX | |
| V34-13 | Low | Code quality | |
| V34-14 | Low | Bug | |
| V34-15 | Low | Accessibility | |
| V34-16 | Low | UX | |
| V34-17 | Low | Code quality | |
| V34-18 | Low | Bug | |
| V34-19 | Low | Security | |
| V34-20 | Low | UX | |
| V34-21 | Low | Performance | |
| V34-22 | Low | Code quality | |
| V34-23 | Low | Documentation | |
| V34-24 | Low | Bug | |
| V34-25 | Low | UX | |

**Total: 25 items** — 4 medium, 21 low
