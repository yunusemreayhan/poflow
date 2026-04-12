# BACKLOG v13 — Fresh Codebase Audit (65 items)

Audit date: 2026-04-12
Codebase: 6117 LOC backend (Rust), 8633 LOC frontend (TS/TSX)
Tests: 239 backend, 154 frontend

---

## Bugs (14)

**B1.** `get_active_timers` holds `engine.states` mutex while doing DB queries (timer.rs:12-19). This blocks the tick loop and all timer operations for all users during the DB round-trips. Fix: collect user IDs under lock, drop lock, then query DB.

**B2.** `edit_comment` parses `created_at` with format `%Y-%m-%dT%H:%M:%S%.3f` but SQLite stores timestamps as `%Y-%m-%dT%H:%M:%S.NNN` (variable fractional digits). If the format doesn't match, the edit window check silently passes (the `if let Ok(...)` falls through). Should use a more lenient parser or store/compare as epoch.

**B3.** `search_tasks_fts` returns `rowid` from the FTS5 table but the FTS5 table is standalone (not external content), so `rowid` is the FTS5 internal row ID — not the task `id`. The INSERT trigger uses `new.id` as the implicit rowid, but this only works if the FTS5 rowid matches. Verify FTS5 rowid == task.id or JOIN back to tasks table.

**B4.** `compare_sprints` loads full `SprintDetail` (including all tasks) just to count completed tasks. This is wasteful for large sprints. Use a COUNT query instead.

**B5.** Rate limiter in `lib.rs` has a duplicate implementation — `api_rate_limit` middleware and `api_limiter()` in `routes/mod.rs` both maintain separate state. The middleware calls `routes::api_limiter()` so they share state, but the cleanup logic is duplicated (lines 212-216 in lib.rs and 53-57 in mod.rs). Remove the duplicate in lib.rs.

**B6.** `export.rs:156` slices `t.title[..50]` for error message on long titles — this will panic on multi-byte UTF-8 titles if byte 50 falls mid-character. Use `t.title.chars().take(50).collect::<String>()`.

**B7.** `SprintParts.tsx` fetches labels for every task in the board view individually (`Promise.all(allIds.map(id => apiCall(...)))`) — this creates N+1 API calls. Should use a batch endpoint or the `/api/tasks/full` response which already includes labels.

**B8.** `ActiveTimers` component in Dashboard.tsx polls every 15s but never cleans up stale data if the API call fails — `setTimers` is only called on success. If the server goes down, the last successful response stays visible indefinitely.

**B9.** `Dependencies.tsx:43-45` has duplicate `aria-label="Add dependency"` attribute on the `<select>` element.

**B10.** `CommentSection.tsx:26` uses `as any` for optimistic comment — the optimistic object is missing `task_id` and `user_id` fields that the `Comment` interface requires. Should type it properly.

**B11.** `instantiate_template` doesn't validate the resolved title length (could exceed 500 chars after variable substitution). Should apply the same 500-char limit as `create_task`.

**B12.** `user_hours_report` date range comparison uses string comparison (`s.started_at >= ? AND s.started_at <= ?`) but `started_at` is a full ISO timestamp, not a date. The `to` parameter should be `to || 'T23:59:59'` to include the full end day.

**B13.** `History.tsx` date filter compares `s.started_at <= dateTo + "T23:59:59"` but `started_at` format from the API may include timezone offset or 'Z' suffix, making string comparison unreliable. Should parse as Date objects.

**B14.** `TaskList.tsx:163` and `:178` use `as any` casts for sort and filter values. Should use proper typed enums.

---

## Security (5)

**S1.** `search_tasks` endpoint doesn't filter by user — any authenticated user can search all tasks including other users' private tasks. Should apply `user_id` filter for non-root users (same as `list_tasks`).

**S2.** `get_task_time_summary` doesn't verify task ownership — any authenticated user can see time tracking details for any task. Should check ownership or at minimum verify the task exists.

**S3.** `get_active_timers` exposes all users' timer state (task titles, timing) to any authenticated user. Consider adding a config flag to disable team visibility or restrict to team members only.

**S4.** `list_labels`, `create_label`, `delete_label` have no authorization checks — any user can create/delete global labels. Labels should either be per-user or require admin role for creation/deletion.

**S5.** `get_all_dependencies`, `get_all_assignees`, `get_all_burn_totals` return data for all users' tasks without filtering. Non-root users should only see data for their own tasks or tasks they're assigned to.

---

## Validation (6)

**V1.** `create_webhook` validates URL but doesn't limit the number of webhooks per user. A user could create thousands of webhooks. Add a per-user limit (e.g., 20).

**V2.** `add_dependency` doesn't check for circular dependencies — task A depends on B, B depends on A. Should walk the dependency chain to detect cycles (similar to parent_id cycle detection in `update_task`).

**V3.** `set_recurrence` doesn't validate that `next_due` is in the future. Setting a past date would cause immediate task creation on the next recurrence check cycle.

**V4.** `create_label` doesn't validate the `color` field format. Should verify it's a valid hex color (e.g., `#[0-9a-fA-F]{6}`).

**V5.** `add_sprint_tasks` allows adding the same task to multiple active sprints. This creates ambiguity for burn tracking (`find_task_active_sprint` returns the first match). Should warn or prevent.

**V6.** `import_tasks_json` recursive `import_tree` function uses `Box::pin` for recursion but doesn't limit total task count across all nesting levels — only checks `req.tasks.len() > 500` at the top level. A deeply nested tree could create far more than 500 tasks.

---

## Tests (10)

**T1.** Test `edit_comment` — create comment, edit within window, verify content changed. Try editing after window expires (mock time or use very old comment), verify 400.

**T2.** Test `search_tasks` — create tasks with distinct titles, search with FTS5 query, verify results contain highlighted snippets with `<mark>` tags.

**T3.** Test `compare_sprints` — create two sprints with different task counts, compare, verify response contains correct counts.

**T4.** Test `get_task_time_summary` — create task, complete sessions, verify hours and per-user breakdown.

**T5.** Test `instantiate_template` — create template with `{{today}}` and `{{username}}` variables, instantiate, verify resolved values in created task.

**T6.** Test `get_active_timers` — start timer for user, verify active timers endpoint returns the user's timer state.

**T7.** Test `change_password` — change password, verify old password no longer works, new password works.

**T8.** Test circular dependency detection — add A→B dependency, try adding B→A, verify 400 error.

**T9.** Test webhook URL validation — try creating webhooks with localhost, private IPs, credential-embedded URLs, verify all rejected.

**T10.** Test label CRUD — create label, add to task, verify task labels, remove, verify empty.

---

## Performance (4)

**P1.** `get_tasks_full` computes ETag from 7 separate COUNT/MAX queries. These could be combined into a single query or cached with a short TTL since this endpoint is called frequently (every SSE reconnect + 30s safety poll).

**P2.** `list_rooms` for non-root users builds the SQL query by string concatenation. Should use a parameterized query builder or at minimum a const SQL string.

**P3.** `TaskDetailView.tsx` fetches labels for each task in the dependency display using `useStore.getState().tasks.find()` inside the render — this is O(n) per dependency. Should build a Map lookup.

**P4.** `SprintParts.tsx` board view re-fetches all task labels on every board change (the `useEffect` depends on `board`). Should cache labels and only fetch for new tasks.

---

## Features (8)

**F1.** Task archiving UI — there's auto-archive logic but no manual archive button or way to view/restore archived tasks in the frontend. Add archive status filter and restore action.

**F2.** Comment edit UI — the `PUT /api/comments/{id}` endpoint exists but the frontend `CommentSection.tsx` has no edit button or inline editing capability.

**F3.** Template instantiation UI — the `POST /api/templates/{id}/instantiate` endpoint exists but the frontend `SettingsParts.tsx` `TemplateManager` only has create/delete, no "Use template" button.

**F4.** Sprint comparison UI — the `GET /api/sprints/compare` endpoint exists but there's no frontend component to select two sprints and display the comparison.

**F5.** Task search results page — the `GET /api/tasks/search` endpoint returns highlighted snippets but the frontend search in `TaskList.tsx` uses the regular `list_tasks` endpoint with `?search=` parameter. Should use the new search endpoint and render `<mark>` highlights.

**F6.** Password change UI — the `PUT /api/auth/password` endpoint exists but there's no frontend form in Settings or Profile to change password.

**F7.** Time summary UI — the `GET /api/tasks/{id}/time-summary` endpoint exists but the `TaskDetailView` doesn't display the per-user time breakdown.

**F8.** Bulk task export from sprint — add "Export as CSV" button to sprint detail view that calls `GET /api/export/burns/{sprint_id}`.

---

## UX (5)

**U1.** Comment timestamps show raw ISO format (`2026-04-12T09:30:00` → `2026-04-12 09:30`). Should use relative time ("2 minutes ago", "yesterday") for recent comments.

**U2.** Task detail breadcrumb navigation doesn't highlight the current task in the tree sidebar. When navigating deep into a task hierarchy, the user loses context of where they are.

**U3.** Sprint board drag-and-drop only works via touch swipe (mobile) — there's no mouse drag support for desktop users to move cards between columns.

**U4.** Empty state for templates — `TemplateManager` shows nothing when there are no templates. Should show a helpful message like "Create a template to quickly add recurring tasks."

**U5.** History heatmap doesn't respond to the date range filter (U4 from v12). The heatmap always shows all data while the session list below is filtered.

---

## Code Quality (5)

**Q1.** `routes/mod.rs` has a large `pub use` block re-exporting everything from every submodule. Several items are only used internally. Should use selective re-exports.

**Q2.** `TaskList.tsx` has 4 `as any` casts that bypass TypeScript's type system. Should define proper union types for sort keys and filter states.

**Q3.** `admin.rs` uses `serde_json::to_vec(...).unwrap()` (lines 59, 112) which can panic if serialization fails. Should use `.map_err(internal)?`.

**Q4.** `engine.rs` tick loop drops and re-acquires the states lock to fetch user configs. This creates a TOCTOU window where a user's state could change between the two lock acquisitions. Document this as acceptable or use a single lock scope.

**Q5.** `webhook.rs` SSRF protection checks individual private IP ranges but misses `fc00::/7` (IPv6 private), `100.64.0.0/10` (CGNAT), and `198.18.0.0/15` (benchmark). Should use a comprehensive private IP check.

---

## Documentation (3)

**D1.** API changelog doesn't document v12 changes (search endpoint, sprint comparison, time summary, comment editing, password change, active timers, template instantiation, T-shirt estimation).

**D2.** No README or CONTRIBUTING guide for the project. New developers have no onboarding documentation.

**D3.** OpenAPI schema is missing several v12 endpoints: `EditCommentRequest`, `SearchQuery`, `CompareQuery`, `ChangePasswordRequest` are not registered in the `components(schemas(...))` block.

---

## Accessibility (3)

**A1.** Dashboard stat cards use `<dl>/<dd>/<dt>` but the `<dl>` wraps a grid of individual stat components — each `Stat` renders its own `<dd>/<dt>` without being inside a proper `<dl>` parent-child relationship. Should restructure.

**A2.** History date inputs (`<input type="date">`) have no associated `<label>` elements. Screen readers can't identify what the date fields are for.

**A3.** Sprint board columns have no ARIA landmarks or headings. Screen readers can't distinguish between "To Do", "In Progress", and "Done" columns.

---

## Cleanup (2)

**C1.** `db/tasks.rs` has `check_fts5` function that's `pub async` but only used internally and always returns `true` (the OnceLock init defaults to true). The actual detection happens in `migrate()`. Remove or make private.

**C2.** `routes/types.rs` defines `VALID_ROLES`, `VALID_ROOM_ROLES`, `VALID_TASK_STATUSES` as module-level constants but they're only used in one file each. Could be moved closer to usage or documented as shared constants.

---

## Summary

| Category       | Count |
|----------------|-------|
| Bugs           | 14    |
| Security       | 5     |
| Validation     | 6     |
| Tests          | 10    |
| Performance    | 4     |
| Features       | 8     |
| UX             | 5     |
| Code Quality   | 5     |
| Documentation  | 3     |
| Accessibility  | 3     |
| Cleanup        | 2     |
| **Total**      | **65** |
