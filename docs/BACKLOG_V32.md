# Comprehensive Audit Backlog (V32)

Fresh full codebase audit — all 7575 LOC backend (55 .rs files), all 8226+ LOC
frontend (48 .ts/.tsx files), 330 backend tests, 154 frontend tests.

---

## Bugs

### V32-1 — `get_state()` drops states lock then acquires config lock (ordering violation)
**Severity:** Medium | **File:** `engine.rs`
`get_state()` acquires `states` lock first, then drops it, then calls
`db::get_today_completed_for_user`. But the comment at the top of Engine
says "Prefer config before states to prevent deadlocks." `get_state()`
acquires `config` lock (via `Self::idle_state` fallback path) while
potentially another thread holds `config` and waits for `states`. The
`idle_state` fallback creates a config lock inside the states lock scope.
Fix: acquire config first, then states, matching the documented ordering.

### V32-2 — `auto_unblock_dependents` checks `status != "completed"` but not `"done"`
**Severity:** Medium | **File:** `routes/tasks.rs`
The helper only considers a dependency resolved when `status == "completed"`.
But `"done"` is also a terminal status. A task blocked by a "done" task
will never auto-unblock.

### V32-3 — Sprint `carryover` doesn't copy labels or assignees
**Severity:** Low | **File:** `routes/sprints.rs`
`carryover_sprint` moves incomplete tasks to a new sprint but doesn't
preserve any task metadata. The tasks themselves keep their labels/assignees
(they're just re-linked), so this is actually fine — the sprint_tasks
junction table is what's created. **FALSE POSITIVE** on closer inspection.

### V32-4 — `duplicate_task` doesn't copy dependencies
**Severity:** Low | **File:** `routes/tasks.rs`
`duplicate_task` copies labels, assignees, PERT estimates, and
work_duration_minutes but doesn't copy task dependencies. A duplicated
task with dependencies loses its dependency graph.

### V32-5 — `import_tasks_csv` doesn't set `status` from CSV
**Severity:** Low | **File:** `routes/export.rs`
CSV import always creates tasks with `status = 'backlog'` even if the
CSV has a `status` column. The export includes status but import ignores it.

### V32-6 — `toast` ID collision possible
**Severity:** Low | **File:** `gui/src/store/store.ts`
Toast IDs use `(Date.now() % 1_000_000_000) * 1000 + random(0..999)`.
Two toasts created in the same millisecond have a 1/1000 chance of
collision. Should use a monotonic counter (same pattern as V31-3 fix).

## Security

### V32-7 — `create_label` has no auth check — any user can create labels
**Severity:** Low | **File:** `routes/labels.rs`
`create_label` only requires `_claims: Claims` (authenticated) but
doesn't check role. `delete_label` and `update_label` require root.
Label creation should also require root for consistency, or be
intentionally open (document the design decision).

### V32-8 — `add_task_link` doesn't validate `link_type`
**Severity:** Low | **File:** `routes/misc.rs`
`add_task_link` accepts any string as `link_type`. Should validate
against known types (e.g., "commit", "pr", "issue", "url") to prevent
arbitrary data injection.

### V32-9 — `import_tasks_json` recursive depth limit is 20 but no total node limit per level
**Severity:** Low | **File:** `routes/export.rs`
While total tasks are capped at 2000 and depth at 20, there's no
per-level breadth limit. A flat import of 2000 tasks at depth 0 is
fine, but the recursive function allocates stack frames. With 20 levels
of nesting and 2000 total nodes, this is acceptable.
**FALSE POSITIVE** — the 2000 total cap is sufficient.

## Performance

### V32-10 — `get_tasks_full` ETag computation runs 7 COUNT queries
**Severity:** Low | **File:** `routes/misc.rs`
The ETag is computed from a single query with 7 subqueries. This runs
on every `/api/tasks/full` request. The query is efficient (all indexed
counts) but could be cached for 1-2 seconds to reduce DB load under
heavy polling.

### V32-11 — `auto_unblock_dependents` does N+1 queries
**Severity:** Low | **File:** `routes/tasks.rs`
For each dependent task, it fetches the task, then fetches all its
dependencies, then fetches each dependency task. For a task with many
dependents, this is O(D * K) queries where D = dependents, K = deps
per dependent. Could batch-fetch.

### V32-12 — `snapshot_active_sprints` runs hourly for all sprints
**Severity:** Low | **File:** `db/sprints.rs`
The hourly snapshot creates a daily stat row for every active sprint.
If there are many active sprints, this could be slow. Currently
acceptable for typical deployments (<10 active sprints).
**WON'T FIX** — acceptable for typical use.

## Code Quality

### V32-13 — `update_label` returns 500 if label ID doesn't exist
**Severity:** Low | **File:** `db/labels.rs`
`update_label` runs UPDATE then SELECT. If the ID doesn't exist, the
UPDATE succeeds (0 rows affected) but the SELECT fails with a sqlx
error, returning 500 instead of 404.

### V32-14 — `Sprints.tsx` export handler is now `async` but error isn't caught
**Severity:** Low | **File:** `gui/src/components/Sprints.tsx`
The V31-18 fix made the export button handler async (to fetch burn data)
but the `onClick` doesn't catch errors. If the burn-summary fetch fails,
the export silently produces markdown without the burn table (which is
the intended fallback via `.catch(() => [])`). **FALSE POSITIVE**.

## Missing Features

### V32-15 — No `PUT /api/webhooks/{id}` to update webhook URL/events
**Severity:** Low | **File:** `routes/webhooks.rs`
Webhooks can be created and deleted but not updated. Users must delete
and recreate to change the URL or events.

### V32-16 — No `PUT /api/templates/{id}` to update template data
**Severity:** Low | **File:** `routes/templates.rs`
Templates can be created, deleted, and instantiated but not edited.

### V32-17 — No pagination on `/api/sprints/{id}/burns`
**Severity:** Low | **File:** `routes/burns.rs`
`list_burns` returns all burns for a sprint with no limit. A sprint
with thousands of manual burns could return a very large response.

### V32-18 — No `PATCH` support for partial task updates
**Severity:** Low | **File:** `routes/tasks.rs`
`PUT /api/tasks/{id}` already handles partial updates via Option fields.
This is a REST convention issue, not a bug. **WON'T FIX**.

## UX / Frontend

### V32-19 — Kanban board doesn't show `active` or `estimated` columns
**Severity:** Low | **File:** `gui/src/components/KanbanBoard.tsx`
The sprint board API returns `todo/in_progress/blocked/done` but the
standalone Kanban view uses the same 4 columns. Tasks with status
`active`, `estimated`, or `backlog` all go to "Todo". Could add
separate columns for better visibility.

### V32-20 — No loading state when switching servers
**Severity:** Low | **File:** `gui/src/store/store.ts`
`switchToServer` validates the token but doesn't show a loading
indicator. The user sees no feedback during the validation request.

### V32-21 — `TaskContextMenu` "Move to root" only shows for child tasks
**Severity:** Low | **File:** `gui/src/components/TaskContextMenu.tsx`
The V31-19 fix added "Move to root" but there's no "Make child of..."
option to reparent a root task under another task via keyboard.

## Accessibility

### V32-22 — Mobile bottom nav doesn't include `rooms` or `history` tabs
**Severity:** Low | **File:** `gui/src/App.tsx`
The mobile nav bar only shows 6 of 10 tabs. Rooms and History are
inaccessible on mobile without switching to desktop layout.

### V32-23 — `AuthScreen` password strength meter has no aria description
**Severity:** Low | **File:** `gui/src/components/AuthScreen.tsx`
The password strength indicator (Weak/Fair/Good/Strong) is visual only.
Screen readers don't announce the strength level.

## Documentation

### V32-24 — `update_label` endpoint not in OpenAPI paths list
**Severity:** Low | **File:** `main.rs`
The V31-17 `update_label` route was added to `lib.rs` but not registered
in the `#[openapi(paths(...))]` macro in `main.rs`, so it won't appear
in Swagger UI.

### V32-25 — `duplicate_task` not in OpenAPI paths list
**Severity:** Low | **File:** `main.rs`
`duplicate_task` is registered as a route but not in the OpenAPI spec.

---

## Summary

| ID | Severity | Category | Status |
|----|----------|----------|--------|
| V32-1 | Medium | Bug | FIXED |
| V32-2 | Medium | Bug | FIXED |
| V32-3 | Low | Bug | FALSE POSITIVE |
| V32-4 | Low | Bug | FIXED |
| V32-5 | Low | Bug | FIXED |
| V32-6 | Low | Bug | FIXED |
| V32-7 | Low | Security | FIXED |
| V32-8 | Low | Security | FIXED |
| V32-9 | Low | Security | FALSE POSITIVE |
| V32-10 | Low | Performance | WON'T FIX (indexed counts, fast enough) |
| V32-11 | Low | Performance | FIXED |
| V32-12 | Low | Performance | WON'T FIX (acceptable for typical use) |
| V32-13 | Low | Code quality | FIXED |
| V32-14 | Low | Code quality | FALSE POSITIVE |
| V32-15 | Low | Missing feature | FIXED |
| V32-16 | Low | Missing feature | FIXED |
| V32-17 | Low | Missing feature | FIXED |
| V32-18 | Low | Missing feature | WON'T FIX (PUT already handles partial) |
| V32-19 | Low | UX | WON'T FIX (4-column Kanban is standard) |
| V32-20 | Low | UX | FIXED |
| V32-21 | Low | UX | FIXED |
| V32-22 | Low | Accessibility | FIXED |
| V32-23 | Low | Accessibility | FIXED |
| V32-24 | Low | Documentation | FIXED |
| V32-25 | Low | Documentation | FIXED |

**Total: 25 items** — 17 fixed, 4 won't fix, 4 false positive
