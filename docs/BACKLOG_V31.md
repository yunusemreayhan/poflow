# Comprehensive Audit Backlog (V31)

Full fresh codebase audit — all 7524 LOC backend (58 .rs files), all 8226 LOC
frontend (48 .ts/.tsx files), 329 backend tests, 154 frontend tests.

---

## Bugs

### V31-1 — `tick()` drops/re-acquires states lock, creating a TOCTOU window
**Severity:** Medium | **File:** `engine.rs`
In `tick()`, the states lock is dropped to fetch per-user configs, then
re-acquired. Between the drop and re-acquire, another request (start/stop)
could modify the state. A user could start a new session during the gap,
and tick would then overwrite it. The window is small but real under load.

### V31-2 — `ErrorBoundary` calls `useI18n.getState()` inside `render()`
**Severity:** Low | **File:** `gui/src/components/ErrorBoundary.tsx`
Class component's `render()` calls `useI18n.getState().t` which is a Zustand
hook accessor. This works but is fragile — if the i18n store isn't initialized
when the error boundary triggers, it could throw during error recovery.

### V31-3 — `CommentSection` optimistic ID uses negative numbers
**Severity:** Low | **File:** `gui/src/components/CommentSection.tsx`
Optimistic comment IDs are `-(Date.now() % 1000000000 + random)`. If the
component re-renders before the server responds, the negative ID could
collide with another optimistic comment. Should use a monotonic counter.

### V31-4 — `recurrence` advance doesn't validate computed next_due
**Severity:** Low | **File:** `db/recurrence.rs`
`advance_recurrence` accepts any string as `next_due` without validating
it's a valid date. The caller (background task in main.rs) computes it
correctly, but the DB function itself has no guard.

## Security

### V31-5 — `get_active_webhooks` LIKE query could match partial events
**Severity:** Low | **File:** `db/webhooks.rs`
The query `events LIKE '%task.updated%'` would also match a hypothetical
event `task.updated_v2`. The LIKE pattern should use comma-delimited
matching or exact event checking.

### V31-6 — No limit on notification count per user
**Severity:** Low | **File:** `db/notifications.rs`
`create_notification` has no cap. A user with many watchers/assignments
could accumulate unbounded notifications. The cleanup runs periodically
but only removes read notifications older than 30 days.

### V31-7 — Attachment storage_key is predictable
**Severity:** Low | **File:** `routes/attachments.rs`
The storage key is `sha256(timestamp+task_id+user_id+size+counter)[:16]_filename`.
While collision-resistant, the inputs are guessable. An attacker who knows
the task_id and upload time could predict the key and access the file
directly if the attachments directory is served statically. Currently
files are served through the API with auth, so this is low risk.

## Performance

### V31-8 — `get_room_state` runs 4+ sequential queries
**Severity:** Low | **File:** `db/rooms.rs`
`get_room_state` uses `tokio::join!` for tasks/current_task/votes but
then runs sequential queries for vote history (title lookup per task).
Could batch the title lookup into a single query.

### V31-9 — `cleanup_orphaned_attachments` reads entire directory
**Severity:** Low | **File:** `db/attachments.rs`
Reads all files in the attachments directory and all storage_keys from DB.
For thousands of attachments, this could be slow. Should run infrequently
(already does — called from background task).

### V31-10 — `list_tasks_paged` builds dynamic SQL with many optional binds
**Severity:** Low | **File:** `db/tasks.rs`
The filter query construction with 9+ optional parameters and dynamic
SQL string building is complex and error-prone. The bind order must
exactly match the SQL clause order. A query builder would be safer.

## Code Quality

### V31-11 — `notify.rs` and `webhook.rs` are separate but tightly coupled
**Severity:** Low | **File:** `notify.rs`, `webhook.rs`
Both handle event dispatch but through different mechanisms (in-app
notifications vs HTTP webhooks). Could share an event bus abstraction.

### V31-12 — Frontend `apiCall` error handling swallows non-GET errors
**Severity:** Low | **File:** `gui/src/store/api.ts`
For non-GET requests, errors are shown as toasts AND re-thrown. But the
toast message parsing (`JSON.parse(msg)`) is in a try/catch that falls
through to `showErrorToast(msg)` on parse failure, potentially showing
raw error strings to users.

## Missing Error Handling

### V31-13 — `advance_recurrence` in background task doesn't handle task creation failure
**Severity:** Low | **File:** `main.rs`
The recurrence background task creates new tasks from recurring templates.
If `create_task` fails (e.g., DB full), the recurrence is still advanced,
so the task is permanently skipped.

### V31-14 — `snapshot_epic_group` silently ignores empty root tasks
**Severity:** Low | **File:** `db/epics.rs`
If all root tasks in an epic group are deleted, `snapshot_epic_group`
returns `Ok(())` without creating a snapshot. The group appears to have
no progress data for that day.

## UX / Frontend

### V31-15 — No visual feedback when timer auto-starts break
**Severity:** Low | **File:** `gui/src/components/Timer.tsx`
When `auto_start_breaks` is enabled, the break starts silently. The
"Session complete!" toast fires but there's no indication that a break
has automatically started vs the timer being idle.

### V31-16 — Estimation room vote history doesn't show timestamps
**Severity:** Low | **File:** `gui/src/components/EstimationRoomView.tsx`
Vote history shows task name, votes, and average but not when the voting
happened. Useful for reviewing estimation sessions.

### V31-17 — No way to edit a label's color after creation
**Severity:** Low | **File:** `gui/src/components/Labels.tsx`, `routes/labels.rs`
Labels can be created with a color and deleted, but there's no PUT
endpoint to update a label's name or color.

### V31-18 — Sprint export markdown doesn't include burn log details
**Severity:** Low | **File:** `gui/src/components/Sprints.tsx`
The "Export" button generates markdown with task list but doesn't include
time logged per task or per-member contribution data.

## Accessibility

### V31-19 — Drag-and-drop in TaskNode has no keyboard alternative
**Severity:** Medium | **File:** `gui/src/components/TaskNode.tsx`
Tasks can be reordered and reparented via drag-and-drop, but there's no
keyboard-accessible way to move tasks. Should add arrow key + modifier
shortcuts or a "Move to" menu.

### V31-20 — Estimation room cards have no focus indicator
**Severity:** Low | **File:** `gui/src/components/EstimationRoomView.tsx`
Vote cards use `onClick` but don't have visible focus rings for keyboard
navigation. Cards should have `tabIndex={0}` and focus styles.

## Documentation

### V31-21 — Webhook event payload schemas not documented
**Severity:** Low | **File:** `docs/CHANGELOG.md`
The CHANGELOG lists webhook events but doesn't document the JSON payload
structure for each event type.

### V31-22 — Recurrence patterns not documented in API
**Severity:** Low | **File:** OpenAPI spec
The valid recurrence patterns (`daily`, `weekly`, `biweekly`, `monthly`)
are defined in code but not in the OpenAPI spec or CHANGELOG.

---

## Summary

| ID | Severity | Category | Status |
|----|----------|----------|--------|
| V31-1 | Medium | Bug | TODO |
| V31-2 | Low | Bug | TODO |
| V31-3 | Low | Bug | TODO |
| V31-4 | Low | Bug | WON'T FIX (caller validates, DB is internal) |
| V31-5 | Low | Security | TODO |
| V31-6 | Low | Security | TODO |
| V31-7 | Low | Security | WON'T FIX (files served through auth API) |
| V31-8 | Low | Performance | WON'T FIX (acceptable for typical room sizes) |
| V31-9 | Low | Performance | WON'T FIX (runs infrequently in background) |
| V31-10 | Low | Code quality | WON'T FIX (refactor, not a bug) |
| V31-11 | Low | Code quality | WON'T FIX (refactor, not a bug) |
| V31-12 | Low | Code quality | TODO |
| V31-13 | Low | Error handling | TODO |
| V31-14 | Low | Error handling | WON'T FIX (empty group is edge case) |
| V31-15 | Low | UX | TODO |
| V31-16 | Low | UX | WON'T FIX (minor, timestamps available in API) |
| V31-17 | Low | UX | TODO |
| V31-18 | Low | UX | TODO |
| V31-19 | Medium | Accessibility | TODO |
| V31-20 | Low | Accessibility | TODO |
| V31-21 | Low | Documentation | TODO |
| V31-22 | Low | Documentation | TODO |

**Total: 22 items** — 2 medium, 20 low
**To fix: 14** | **Won't fix: 8** (by design, refactors, or acceptable tradeoffs)
