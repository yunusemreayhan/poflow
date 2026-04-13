# Backlog V38 — Full Codebase Audit (2026-04-13)

Scope: Stability, correctness, security, performance, UX, accessibility, code quality.
No new features.

---

## V38-1 [Medium / Bug] 4 watcher endpoints missing from OpenAPI paths registration
**File:** `main.rs` paths(...)
`get_task_watchers`, `get_watched_tasks`, `watch_task`, `unwatch_task` are registered in the router (`lib.rs`) and have `#[utoipa::path]` annotations but are not listed in the `paths(...)` macro in `main.rs`. They won't appear in Swagger UI or the OpenAPI spec.

## V38-2 [Medium / Bug] SSE `JSON.parse` without try/catch can crash connection
**File:** `gui/src/hooks/useSseConnection.ts:32,57`
`JSON.parse(e.data)` is called without try/catch. If the server sends malformed SSE data (e.g. during a partial write or network corruption), the entire SSE connection handler crashes and the client loses real-time updates silently.

## V38-3 [Medium / Bug] `activeTeamId` localStorage parse without try/catch
**File:** `gui/src/store/store.ts:129`
`JSON.parse(localStorage.getItem("activeTeamId") || "null")` — if localStorage contains invalid JSON (e.g. corrupted by another extension), the entire store initialization crashes and the app won't load.

## V38-4 [Medium / Security] `legacy XOR decryption` in webhook secrets still active
**File:** `db/webhooks.rs:decrypt_secret_xor`
The legacy XOR decryption fallback is still present for backwards compatibility. XOR with a repeating key is trivially breakable. Old webhook secrets encrypted with XOR should be re-encrypted with AES-GCM on first successful decryption.

## V38-5 [Medium / Bug] `carryover_sprint` doesn't copy sprint project/goal to new sprint
**File:** `routes/sprints.rs:carryover_sprint`
The carry-over creates a new sprint with `sprint.project` and `sprint.goal` from the original, but `capacity_hours` is also copied. However, the new sprint name is `"{name} (carry-over)"` which could accumulate: `"Sprint 1 (carry-over) (carry-over)"`. Should strip existing `(carry-over)` suffix.

## V38-6 [Low / Bug] `export_tasks` CSV header doesn't match V36-18 additions
**File:** `routes/export.rs:export_tasks`
The CSV export header includes `estimated_hours` and `remaining_points` (added in V36-18), but the `import_tasks_csv` parser needs to be verified to handle these columns correctly on re-import.

## V38-7 [Low / Code Quality] `estimation_accuracy` builds SQL string with conditional concatenation
**File:** `routes/history.rs:estimation_accuracy`
Uses `sql.push_str(" AND user_id = ?")` pattern with conditional binds. While safe (parameterized), this pattern is fragile — adding a new filter could misalign bind order. Consider using a query builder.

## V38-8 [Low / Bug] `get_sprint_detail` doesn't return 404 for non-existent sprint
**File:** `routes/sprints.rs:get_sprint_detail`
Calls `db::get_sprint_detail` which calls `db::get_sprint` internally. If the sprint doesn't exist, `get_sprint` returns an `Err` which gets mapped to 500 via `internal()` instead of 404.

## V38-9 [Low / Bug] `snapshot_sprint` and `snapshot_epic_group` don't validate date uniqueness
**File:** `routes/sprints.rs`, `routes/epics.rs`
The DB has `UNIQUE(sprint_id, date)` / `UNIQUE(group_id, date)` constraints. If a snapshot is taken twice on the same day, the second call returns a 500 (UNIQUE constraint violation) instead of a meaningful error or upsert.

## V38-10 [Low / UX] Calendar view buttons lack `type="button"` — may submit forms
**File:** `gui/src/components/CalendarView.tsx`
Calendar day cells are `<button>` elements inside the component. While there's no enclosing `<form>`, if the component is ever nested in a form context, these buttons would trigger form submission. Best practice: add `type="button"`.

## V38-11 [Low / Code Quality] `Hmac::new_from_slice().unwrap()` in `webhook.rs` dispatch
**File:** `webhook.rs:80`
`<Hmac<Sha256>>::new_from_slice(secret.as_bytes()).unwrap()` — HMAC-SHA256 accepts any key length so this can't actually fail, but the `unwrap()` is inconsistent with the rest of the codebase which now avoids bare unwraps.

## V38-12 [Low / Bug] `get_task_detail` returns 500 instead of 404 for non-existent task
**File:** `routes/tasks.rs:get_task_detail`
`db::get_task_detail(&engine.pool, id).await.map(Json).map_err(internal)` — if the task doesn't exist, the DB error is mapped to 500 via `internal()`. Should map to 404.

## V38-13 [Low / Bug] `list_sprints` doesn't validate `status` query parameter
**File:** `routes/sprints.rs:list_sprints`
The `status` filter is passed directly to the DB query without validation. Invalid status values like `status=foo` silently return empty results instead of a 400 error.

## V38-14 [Low / Accessibility] Kanban board drag-and-drop has no keyboard alternative for moving between columns
**File:** `gui/src/components/KanbanBoard.tsx`
The `KanbanCard` has `onKeyDown` for Enter to advance to next status, but there's no way to move a task to an arbitrary column via keyboard (e.g. move from "Active" to "Blocked"). Only the "natural" next status is available.

## V38-15 [Low / Code Quality] `OpenAPI version` should be bumped after V37/V38 changes
**File:** `main.rs:119`
Currently `2.0.1`. Should be updated to reflect the V37+V38 endpoint additions (watcher endpoints, schema additions).

## V38-16 [Low / Bug] `delete_user` doesn't stop active timer for deleted user
**File:** `routes/admin.rs:delete_user`
If a user has an active timer running when they're deleted, the timer state remains in `engine.states` HashMap. The orphaned state will tick forever, trying to write sessions for a non-existent user.

## V38-17 [Low / Bug] `import_tasks_csv` doesn't validate `estimated_hours` and `remaining_points` columns
**File:** `routes/export.rs:import_tasks_csv`
Need to verify the CSV import handles the new columns added in V36-18. If the import expects the old format, importing a V36+ export would fail or misalign columns.

## V38-18 [Low / Performance] `get_all_task_labels` called on every `get_tasks_full` request
**File:** `routes/misc.rs:get_tasks_full`
The ETag check prevents re-serializing the response, but the 5 parallel DB queries still execute even when the ETag matches. Should check ETag before running queries.

## V38-19 [Low / Code Quality] `EditCommentRequest` not registered in OpenAPI schemas
**File:** `routes/comments.rs`, `main.rs`
`EditCommentRequest` is used by `edit_comment` but isn't in the `components(schemas(...))` list.

## V38-20 [Low / Bug] `sprint_retro_report` doesn't check sprint existence
**File:** `routes/sprints.rs` or `routes/teams.rs`
Need to verify if `sprint_retro_report` returns 404 for non-existent sprints or silently returns empty data.

---

## Summary

| ID | Severity | Category | Status |
|----|----------|----------|--------|
| V38-1 | Medium | Bug | FIXED — 4 watcher endpoints registered in OpenAPI paths |
| V38-2 | Medium | Bug | FALSE POSITIVE — JSON.parse already wrapped in try/catch |
| V38-3 | Medium | Bug | FIXED — activeTeamId JSON.parse wrapped in try/catch |
| V38-4 | Medium | Security | FIXED — migrate_legacy_secrets re-encrypts XOR to AES-GCM at startup |
| V38-5 | Medium | Bug | FIXED — strips existing (carry-over) suffix |
| V38-6 | Low | Bug | FALSE POSITIVE — import uses dynamic col_idx mapping |
| V38-7 | Low | Code quality | WON'T FIX — pattern is safe with parameterized queries |
| V38-8 | Low | Bug | FIXED — returns 404 instead of 500 |
| V38-9 | Low | Bug | FIXED — returns 409 Conflict for duplicate snapshots |
| V38-10 | Low | UX | WON'T FIX — buttons not inside any form |
| V38-11 | Low | Code quality | WON'T FIX — HMAC-SHA256 new_from_slice cannot fail |
| V38-12 | Low | Bug | FIXED — returns 404 instead of 500 |
| V38-13 | Low | Bug | FIXED — validates status query parameter |
| V38-14 | Low | Accessibility | WON'T FIX — full column navigation would be a new feature |
| V38-15 | Low | Code quality | FIXED — OpenAPI version bumped to 2.1.0 |
| V38-16 | Low | Bug | FIXED — removes deleted user's timer state |
| V38-17 | Low | Bug | FALSE POSITIVE — import handles estimated_hours/remaining_points |
| V38-18 | Low | Performance | FALSE POSITIVE — ETag check already runs before heavy queries |
| V38-19 | Low | Code quality | FIXED — EditCommentRequest registered in OpenAPI schemas |
| V38-20 | Low | Bug | FALSE POSITIVE — sprint_retro_report checks existence with 404 |

**Total: 20 items** — 11 fixed, 4 won't fix, 5 false positive
