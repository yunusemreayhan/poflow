# Backlog V40 — Full Codebase Audit (2026-04-13)

Scope: Stability, correctness, security, performance, UX, accessibility, code quality.
No new features.

---

## V40-1 [Medium / Bug] `list_task_burns` has no LIMIT — unbounded response
**File:** `db/burns.rs:33-35`
`list_task_burns` returns all burn entries for a task with no LIMIT. `list_burns` (sprint burns) has LIMIT 1000, but the per-task equivalent doesn't. A task with thousands of burns would produce an unbounded response.

## V40-2 [Medium / Bug] `end_session` duration can be negative if system clock goes backwards
**File:** `db/sessions.rs:27`
`let duration = (Utc::now().naive_utc() - started).num_seconds()` — if the system clock is adjusted backwards (NTP correction, manual change), `duration` can be negative. This negative value gets stored in the DB and propagates to stats/reports.

## V40-3 [Medium / Bug] `recover_interrupted` doesn't clear engine states for recovered sessions
**File:** `main.rs`, `db/sessions.rs`
On startup, `recover_interrupted` marks all `running` sessions as `interrupted` in the DB. But if the engine had in-memory states from a previous run (not possible with fresh start, but relevant if the recovery logic is called at other times), those states would be stale.

## V40-4 [Medium / Bug] `get_history` CTE can produce infinite loop if task parent_id has a cycle
**File:** `db/sessions.rs:get_history`
The recursive CTE `WITH RECURSIVE ancestors AS (... JOIN ancestors a ON t.id = a.parent_id)` has no depth limit. If a task's parent_id chain has a cycle (shouldn't happen due to V7 validation, but could exist from pre-V7 data), this query would loop until SQLite's recursion limit.

## V40-5 [Low / Bug] `snapshot_sprint` route-level 409 check (V38-9) is dead code
**File:** `routes/teams.rs:snapshot_sprint`
The DB function uses `ON CONFLICT ... DO UPDATE` (upsert), so the UNIQUE constraint error that the route checks for can never occur. The 409 check is harmless but misleading.

## V40-6 [Low / Bug] `delete_label` doesn't return 404 for non-existent label
**File:** `routes/labels.rs:delete_label`, `db/labels.rs:delete_label`
`DELETE FROM labels WHERE id = ?` silently succeeds even if the label doesn't exist. Should check rows_affected.

## V40-7 [Low / Bug] `remove_dependency` doesn't return 404 for non-existent dependency
**File:** `routes/dependencies.rs:remove_dependency`, `db/dependencies.rs`
Same pattern — silent success on non-existent dependency.

## V40-8 [Low / Bug] `remove_task_label` doesn't return 404 for non-existent task-label association
**File:** `routes/labels.rs:remove_task_label`
Silent success when removing a label that isn't on the task.

## V40-9 [Low / Code Quality] `recover_interrupted_sessions` is a dead wrapper function
**File:** `db/sessions.rs:recover_interrupted_sessions`
Wraps `recover_interrupted` and converts `Vec<Session>` to `u64` length. But it's never called — only `recover_interrupted` is used directly.

## V40-10 [Low / Bug] `get_task_detail` fetches all descendant sessions without LIMIT
**File:** `db/comments.rs:get_task_detail`
The batch session fetch `WHERE s.task_id IN (...)` has no LIMIT. A task tree with many sessions could produce a very large response.

## V40-11 [Low / Code Quality] `epic_snapshots` uses `ON CONFLICT ... DO UPDATE` but V38-9 added route-level 409 check
**File:** `routes/epics.rs:snapshot_epic_group`
Same as V40-5 — the 409 check is dead code because the DB uses upsert.

## V40-12 [Low / Bug] `add_comment` doesn't validate `parent_id` exists or belongs to same task
**File:** `routes/comments.rs:add_comment`
If `parent_id` is provided for a threaded reply, there's no validation that the parent comment exists or belongs to the same task. A user could create a reply pointing to a comment on a different task.

## V40-13 [Low / Bug] `get_task_sessions` doesn't check task existence
**File:** `routes/tasks.rs:get_task_sessions`
Returns empty for non-existent task instead of 404. Same pattern as V39-7/8/9 but for sessions.

## V40-14 [Low / Bug] `list_comments` doesn't check task existence
**File:** `routes/comments.rs:list_comments`
Returns empty for non-existent task instead of 404.

## V40-15 [Low / Bug] `list_attachments` doesn't check task existence
**File:** `routes/attachments.rs:list_attachments`
Returns empty for non-existent task instead of 404.

---

## Summary

| ID | Severity | Category | Status |
|----|----------|----------|--------|
| V40-1 | Medium | Bug | |
| V40-2 | Medium | Bug | |
| V40-3 | Medium | Bug | FALSE POSITIVE — recovery runs before engine init and at shutdown |
| V40-4 | Medium | Bug | |
| V40-5 | Low | Bug | |
| V40-6 | Low | Bug | |
| V40-7 | Low | Bug | |
| V40-8 | Low | Bug | |
| V40-9 | Low | Code quality | |
| V40-10 | Low | Bug | |
| V40-11 | Low | Code quality | |
| V40-12 | Low | Bug | |
| V40-13 | Low | Bug | |
| V40-14 | Low | Bug | |
| V40-15 | Low | Bug | |

**Total: 15 items** — 4 medium, 11 low (1 pre-marked false positive)
