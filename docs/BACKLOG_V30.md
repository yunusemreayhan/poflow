# Comprehensive Audit Backlog (V30)

Full fresh codebase audit — all 7524 LOC backend (58 .rs files), all 8226 LOC
frontend (48 .ts/.tsx files), all 150+ route handlers, 16 DB migrations, all
frontend components, store, hooks, and utilities.

---

## Bugs

### V30-1 — `delete_user` doesn't clean up `achievements` or `automation_rules`
**Severity:** Medium | **File:** `db/users.rs`
The `delete_user` transaction cleans up 15+ tables but misses `achievements`,
`automation_rules`, `session_participants`, and `task_links`. These have
`ON DELETE CASCADE` on the FK, but `task_links` doesn't have a user FK at all
and `automation_rules` does — so the CASCADE handles it. However,
`session_participants` references `users(id)` with CASCADE, so it's fine.
Only `task_templates` is explicitly deleted but `task_links` created by the
user's webhook actions would be orphaned (no user_id column on task_links).
**Actual issue:** `task_links` has no user_id — links created via GitHub
webhook for a deleted user's tasks remain but are harmless. **Low risk.**

### V30-2 — `edit_comment` timestamp parsing doesn't handle timezone offset
**Severity:** Low | **File:** `routes/comments.rs`
`NaiveDateTime::parse_from_str` with `%Y-%m-%dT%H:%M:%S%.f` will fail if
the DB ever stores timestamps with timezone offsets. Currently `now_str()`
always produces naive UTC, so this is safe — but fragile if the format changes.

### V30-3 — `bulk_update_status` doesn't validate status value
**Severity:** Low | **File:** `routes/tasks.rs`
`update_task` validates status against `VALID_STATUSES`, but
`bulk_update_status` passes `req.status` directly to `db::update_task`
without checking it's a valid status first.

### V30-4 — `restore_task` doesn't re-index FTS5
**Severity:** Low | **File:** `db/tasks.rs`
When a task is soft-deleted, the FTS trigger removes it. When restored via
`restore_task`, the task's `deleted_at` is set to NULL, but the FTS UPDATE
trigger only re-inserts when `new.deleted_at IS NULL` — this works because
the trigger fires on UPDATE. **Actually OK** — the trigger handles it.
**FALSE POSITIVE.**

### V30-5 — `get_descendant_ids` depth limit of 50 is silent
**Severity:** Low | **File:** `db/epics.rs`
The recursive CTE has `WHERE d.depth < 50` but doesn't warn if the limit
is hit. A task tree deeper than 50 levels would silently truncate.

### V30-6 — Sprint `complete` doesn't auto-snapshot
**Severity:** Low | **File:** `routes/sprints.rs`
When completing a sprint, the final state isn't automatically snapshotted.
Users must manually click "Snapshot" before completing. Should auto-snapshot
on completion to capture final burndown data point.

### V30-7 — `CalendarView` arrow key navigation doesn't account for header row
**Severity:** Low | **File:** `gui/src/components/CalendarView.tsx`
The V29-19 arrow key handler indexes into `cells` but the grid also contains
7 header cells (day names). The `target.parentElement.children` includes
headers, so arrow navigation may jump to wrong cells.

## Security

### V30-8 — `seed_root_user` logs generated password to stdout
**Severity:** Medium | **File:** `db/users.rs`
When no `POMODORO_ROOT_PASSWORD` is set, the generated password is logged
via `tracing::warn!`. In production with JSON logging, this password ends
up in log files. Should only log to stderr on first run, or write to a
file with restricted permissions.

### V30-9 — Webhook secret stored in plaintext
**Severity:** Low | **File:** `db/webhooks.rs`
User webhook secrets are stored as plaintext in the `webhooks` table.
Should be hashed (like passwords) since they're used for HMAC verification.

### V30-10 — No rate limiting on comment creation
**Severity:** Low | **File:** `routes/comments.rs`
Comments have content validation (empty, max 10000 chars) but no rate
limiting. A user could spam thousands of comments rapidly. The auth rate
limiter only covers login attempts.

### V30-11 — `admin_reset_password` doesn't require current admin password
**Severity:** Low | **File:** `routes/admin.rs`
Root users can reset any user's password without re-authenticating. If a
root session token is stolen, the attacker can change all passwords.
Should require the admin's current password for this sensitive operation.

## Performance

### V30-12 — `loadTasks` fetches ALL tasks on every SSE change event
**Severity:** Medium | **File:** `gui/src/store/store.ts`
Every SSE "Tasks" change event triggers a full `/api/tasks/full` reload.
With many tasks, this is expensive. Should use `If-Modified-Since` or
ETag-based caching, or send only changed task IDs in the SSE event.

### V30-13 — `get_task_detail` CTE loads all descendants even for leaf tasks
**Severity:** Low | **File:** `db/comments.rs`
For leaf tasks (no children), the recursive CTE still runs. Minor overhead
but could short-circuit if the task has no children.

### V30-14 — `snapshot_epic_group` iterates all groups sequentially
**Severity:** Low | **File:** `db/epics.rs`
`snapshot_all_epic_groups` snapshots each group one at a time. Could use
`tokio::join!` or batch SQL for better performance with many groups.

### V30-15 — No pagination on `list_comments`
**Severity:** Low | **File:** `db/comments.rs`
Comments are fetched without LIMIT. A task with thousands of comments
would return all of them. Should add pagination or a reasonable limit.

## Code Quality

### V30-16 — `routes/mod.rs` has 147 lines of imports and re-exports
**Severity:** Low | **File:** `routes/mod.rs`
The module file is mostly boilerplate. Could use a macro or wildcard
re-exports to reduce maintenance burden.

### V30-17 — Inconsistent error handling: some routes use `internal()`, others use `err()`
**Severity:** Low | **File:** Various routes
Some routes return `internal(e)` for all errors (losing the specific
status code), while others properly map to 404/400/403. The pattern is
inconsistent across files.

### V30-18 — `store.ts` is 457 lines with all state + actions in one object
**Severity:** Low | **File:** `gui/src/store/store.ts`
The Zustand store has grown large. Could be split into slices (auth,
timer, tasks, ui) for better maintainability.

## Missing Error Handling

### V30-19 — `switchToServer` doesn't validate the saved token
**Severity:** Low | **File:** `gui/src/store/store.ts`
When switching to a saved server, the stored token is used directly
without checking if it's still valid. If expired, the user sees API
errors instead of being redirected to login.

### V30-20 — `instantiate_template` doesn't validate template data structure
**Severity:** Low | **File:** `routes/templates.rs`
Template data is parsed as `serde_json::Value` but fields like `priority`
are accessed with `as_i64()` which returns None for invalid types. A
template with `"priority": "high"` would silently default to 3.

## UX / Frontend

### V30-21 — No confirmation before leaving a room
**Severity:** Low | **File:** `gui/src/components/EstimationRoomView.tsx`
Clicking "Leave" immediately leaves the room without confirmation.
Should show a confirm dialog, especially during active voting.

### V30-22 — Sprint burndown chart has no data point labels
**Severity:** Low | **File:** `gui/src/components/SprintViews.tsx`
The burndown SVG chart shows lines but no hover tooltips or data labels.
Users can't see exact values for specific dates.

### V30-23 — No visual indicator for tasks with dependencies
**Severity:** Low | **File:** `gui/src/components/TaskNode.tsx`
Tasks with unresolved dependencies show no icon in the tree view.
Only the sprint board shows blocked-by info. The task tree should
show a 🔗 or similar indicator.

### V30-24 — Timer doesn't show PERT expected duration
**Severity:** Low | **File:** `gui/src/components/Timer.tsx`
Tasks with PERT estimates (optimistic/pessimistic) don't show the
calculated expected time `(O + 4M + P) / 6` anywhere in the timer view.

### V30-25 — No keyboard shortcut to switch tabs
**Severity:** Low | **File:** `gui/src/App.tsx`
The app has keyboard shortcuts for timer (Space) and search (/), but
no shortcuts to switch between Timer/Tasks/Sprints/etc tabs.

## Accessibility

### V30-26 — Color-only status indicators in sprint board
**Severity:** Low | **File:** `gui/src/components/SprintParts.tsx`
Sprint board columns use color alone to distinguish status (green for
active, blue for completed). Should add text labels or icons for
color-blind users.

### V30-27 — Toast notifications lack ARIA live region
**Severity:** Low | **File:** `gui/src/App.tsx`
Toast messages appear visually but may not be announced by screen
readers. Should use `role="alert"` or `aria-live="polite"`.

## Documentation

### V30-28 — No API error response documentation
**Severity:** Low | **File:** OpenAPI spec
The OpenAPI spec documents success responses but not error responses
(400, 401, 403, 404, 500). Should add error response schemas.

### V30-29 — No developer setup guide
**Severity:** Low | **File:** Project root
No README section on how to set up the development environment, run
tests, or configure the database. New contributors would struggle.

---

## Summary

| ID | Severity | Category | Status |
|----|----------|----------|--------|
| V30-1 | Medium | Bug | WON'T FIX (CASCADE handles it, task_links harmless) |
| V30-2 | Low | Bug | WON'T FIX (format is consistent, fragility is theoretical) |
| V30-3 | Low | Bug | FALSE POSITIVE (already calls validate_task_status) |
| V30-4 | — | Bug | FALSE POSITIVE |
| V30-5 | Low | Bug | WON'T FIX (50-level depth is extreme edge case) |
| V30-6 | Low | Bug | FALSE POSITIVE (already auto-snapshots before completing) |
| V30-7 | Low | Bug | FALSE POSITIVE (headers are in separate container) |
| V30-8 | Medium | Security | FIXED |
| V30-9 | Low | Security | WON'T FIX (webhook secrets are user-provided, not passwords) |
| V30-10 | Low | Security | FIXED |
| V30-11 | Low | Security | WON'T FIX (root is trusted, re-auth adds friction) |
| V30-12 | Medium | Performance | FIXED |
| V30-13 | Low | Performance | WON'T FIX (CTE overhead is negligible for leaf tasks) |
| V30-14 | Low | Performance | WON'T FIX (sequential is fine for <100 groups) |
| V30-15 | Low | Performance | FIXED |
| V30-16 | Low | Code quality | WON'T FIX (refactor, not a bug) |
| V30-17 | Low | Code quality | WON'T FIX (refactor, not a bug) |
| V30-18 | Low | Code quality | WON'T FIX (refactor, not a bug) |
| V30-19 | Low | Error handling | FIXED |
| V30-20 | Low | Error handling | FIXED |
| V30-21 | Low | UX | FIXED |
| V30-22 | Low | UX | WON'T FIX (SVG chart is intentionally minimal) |
| V30-23 | Low | UX | FIXED |
| V30-24 | Low | UX | FIXED |
| V30-25 | Low | UX | FALSE POSITIVE (keys 0-7 already switch tabs) |
| V30-26 | Low | Accessibility | FALSE POSITIVE (columns have text titles) |
| V30-27 | Low | Accessibility | FALSE POSITIVE (already has aria-live) |
| V30-28 | Low | Documentation | FIXED |
| V30-29 | Low | Documentation | FIXED |

**Total: 29 items** — 10 fixed, 7 false positives, 12 won't fix
