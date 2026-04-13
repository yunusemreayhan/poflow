# Backlog V42 — Full Codebase Audit (2026-04-13)

Scope: Stability, correctness, security, performance, UX, accessibility, code quality.
No new features.

---

## V42-1 [Medium / Bug] `get_sprint_tasks` doesn't check sprint existence — returns empty instead of 404
**File:** `routes/sprints.rs:153-154`
Same pattern as previous audits. Returns `[]` for non-existent sprint IDs.

## V42-2 [Medium / Bug] `get_sprint_burndown` doesn't check sprint existence — returns empty instead of 404
**File:** `routes/sprints.rs:get_sprint_burndown`
Returns `[]` for non-existent sprint IDs.

## V42-3 [Medium / Bug] `remove_sprint_task` silently succeeds if task isn't in the sprint
**File:** `db/sprints.rs:65-68`, `routes/sprints.rs:remove_sprint_task`
The DELETE silently succeeds even if the task_id isn't associated with the sprint. Should check rows_affected and return 404.

## V42-4 [Medium / Bug] `remove_assignee` route maps task-not-found to 500 instead of 404
**File:** `routes/assignees.rs:25`
`db::get_task(&engine.pool, id).await.map_err(internal)` — if the task doesn't exist, this returns 500 instead of 404.

## V42-5 [Low / Bug] `list_assignees` doesn't check task existence — returns empty instead of 404
**File:** `routes/assignees.rs:4-6`
Returns `[]` for non-existent task IDs.

## V42-6 [Low / Bug] `remove_assignee` DB function silently succeeds if user isn't assigned
**File:** `db/assignees.rs:12-15`
The DELETE silently succeeds. Should check rows_affected.

## V42-7 [Low / Bug] `get_global_burndown` returns all sprint daily stats without LIMIT
**File:** `routes/sprints.rs:get_global_burndown`
No LIMIT on the global burndown query. With many sprints over time, this could return thousands of rows.

## V42-8 [Low / Code Quality] `update_config` locks `engine.config` twice for root users
**File:** `routes/config.rs:update_config`
For root users, the code does `engine.config.lock().await` to read network settings, drops it, then `engine.config.lock().await` again to write. Could be a single lock scope.

## V42-9 [Low / Bug] `task_added_to_sprint` notification kind not in EVENT_TYPES
**File:** `routes/sprints.rs:add_sprint_tasks`, `routes/profile.rs:EVENT_TYPES`
The notification kind `"task_added_to_sprint"` is used in `add_sprint_tasks` but isn't in the `EVENT_TYPES` list, so users can't configure preferences for it.

## V42-10 [Low / Bug] `get_velocity` doesn't validate negative `sprints` query param
**File:** `routes/sprints.rs:get_velocity`
`q.sprints.unwrap_or(10).min(50)` — a negative value like `-5` would pass through to the DB query. Should clamp to at least 1.

---

## Summary

| ID | Severity | Category | Status |
|----|----------|----------|--------|
| V42-1 | Medium | Bug | FIXED — returns 404 for non-existent sprint |
| V42-2 | Medium | Bug | FIXED — returns 404 for non-existent sprint |
| V42-3 | Medium | Bug | FIXED — returns 404 if task not in sprint |
| V42-4 | Medium | Bug | FIXED — returns 404 instead of 500 |
| V42-5 | Low | Bug | FIXED — returns 404 for non-existent task |
| V42-6 | Low | Bug | FIXED — returns 404 if user not assigned |
| V42-7 | Low | Bug | WON'T FIX — query scoped to active sprints, bounded by date count |
| V42-8 | Low | Code quality | FIXED — single lock scope |
| V42-9 | Low | Bug | FIXED — added to EVENT_TYPES |
| V42-10 | Low | Bug | FIXED — clamped to 1-50 |

**Total: 10 items** — 9 fixed, 1 won't fix
