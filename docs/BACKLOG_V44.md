# Backlog V44 — Full Codebase Audit (2026-04-13)

Scope: Stability, correctness, security, performance, UX, accessibility, code quality.
No new features.

---

## V44-1 [Medium / Bug] `get_history` doesn't validate `from`/`to` date format
**File:** `routes/history.rs:get_history`
The `from` and `to` query params are passed directly to the DB without format validation. `export_sessions` validates them, but `get_history` doesn't. Malformed dates would silently return no results.

## V44-2 [Medium / Bug] `remove_epic_group_task` silently succeeds if task isn't in the group
**File:** `db/epics.rs:remove_epic_group_task`, `routes/epics.rs:62`
The DELETE silently succeeds even if the task_id isn't associated with the epic group. Should check rows_affected.

## V44-3 [Medium / Bug] `remove_sprint_root_task` silently succeeds if task isn't a root task
**File:** `db/epics.rs:remove_sprint_root_task`, `routes/epics.rs:101`
Same pattern — DELETE silently succeeds for non-existent association.

## V44-4 [Low / Bug] `get_sprint_scope` doesn't check sprint existence
**File:** `routes/epics.rs:get_sprint_scope`
Returns empty for non-existent sprint instead of 404.

## V44-5 [Low / Bug] `leaderboard` doesn't validate `period` param — unknown values silently default to 7 days
**File:** `routes/history.rs:leaderboard`
`match q.period.as_deref() { Some("month") => 30, Some("year") => 365, _ => 7 }` — any unknown value like `"decade"` silently defaults to 7 days. Should return 400 for invalid values.

## V44-6 [Low / Bug] `activity_feed` `types` param accepts any value without validation
**File:** `routes/history.rs:activity_feed`
The `types` filter accepts any comma-separated values. Unknown types like `"foo"` silently return no results for that type. Should validate against known types.

## V44-7 [Low / Code Quality] `estimation_accuracy` SQL builds dynamic WHERE with string concatenation
**File:** `routes/history.rs:estimation_accuracy`
Uses `sql.push_str(" AND user_id = ?")` pattern. While safe (parameterized), it's the only analytics endpoint using this pattern — all others use static SQL.

## V44-8 [Low / Bug] `duplicate_task` copies `sort_order` which may cause ordering conflicts
**File:** `routes/tasks.rs:duplicate_task`
When duplicating a task, the copy gets the same `sort_order` as the original. Two tasks with identical sort_order in the same parent creates ambiguous ordering.

---

## Summary

| ID | Severity | Category | Status |
|----|----------|----------|--------|
| V44-1 | Medium | Bug | |
| V44-2 | Medium | Bug | |
| V44-3 | Medium | Bug | |
| V44-4 | Low | Bug | |
| V44-5 | Low | Bug | |
| V44-6 | Low | Bug | |
| V44-7 | Low | Code quality | |
| V44-8 | Low | Bug | |

**Total: 8 items** — 3 medium, 5 low
