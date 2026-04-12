# BACKLOG v24 — Fresh Codebase Audit (2026-04-12)

Full audit of 58 backend .rs files (~6800 LOC), 66 frontend .ts/.tsx files (~9300 LOC), 275 backend tests, 154 frontend tests.

## Security (1 item)

- [ ] **S1.** `TaskContextMenu` "Save as template" passes `JSON.stringify(data)` as the template `data` field, but the backend `create_template` expects `data` to be a JSON value (`serde_json::Value`), not a string. The template is stored as a double-encoded JSON string (`"\"{ ... }\""` instead of `{ ... }`). When `instantiate_template` later does `serde_json::from_str(&tmpl.data)`, it gets the outer string, not the inner object. The `data["title"]` lookup returns `None`, falling back to `tmpl.name`. This means template variable resolution (`{{today}}`, `{{username}}`) doesn't work for templates created via the context menu.

## Bugs (3 items)

- [ ] **B1.** `delete_user` in `db/users.rs` reassigns tasks/sessions/sprints to `(SELECT id FROM users WHERE role = 'root' AND id != ? LIMIT 1)`. But if the deleted user IS the last non-self root user, this subquery returns NULL, and the UPDATE silently sets `user_id = NULL` which violates the NOT NULL constraint on `tasks.user_id`. The `delete_user` function checks for last root user deletion, but doesn't handle the case where reassignment target is NULL.

- [ ] **B2.** `CommentSection` optimistic update creates a comment with `id: -(Date.now() % 1000000000 + ...)`. After `addComment` resolves and `load()` is called, the real comments replace the optimistic ones. But if `load()` fails (network error), the optimistic comment with negative ID stays in the list permanently until the component remounts.

- [ ] **B3.** `get_sprint_board` maps `"active"` status to `in_progress` column, but `BoardView` keyboard navigation uses `statusOrder = ["backlog", "in_progress", "blocked", "completed"]`. When a user presses ArrowRight on a task in the "In Progress" column, it changes status to `"blocked"`. But the board maps `"active"` → in_progress, so tasks with status `"active"` would be moved to `"blocked"` instead of staying in the expected flow. The board should normalize `"active"` to `"in_progress"` when changing status.

## Validation (1 item)

- [ ] **V1.** `InlineTimeReport` allows submitting with `hours = 0.25` minimum (via `min="0.25"` on the input), but the backend `add_time_report` only checks `hours > 0`. The HTML `min` attribute is client-side only and can be bypassed. Not a real issue since the backend validates, but the frontend `submit` function checks `!h || h <= 0` which would allow `0.01` hours (36 seconds) — probably too small to be meaningful.

## Code Quality (2 items)

- [ ] **CQ1.** `TaskNode` component is 280+ lines with 20+ state variables. It handles rendering, drag-and-drop, context menu, inline editing, commenting, time reporting, and keyboard shortcuts all in one component. This makes it hard to maintain and test. Should be split into smaller focused components.

- [ ] **CQ2.** `get_descendant_ids` is called in multiple places (`delete_task`, `restore_task`, `list_tasks_paged` with team scope, `get_sprint_scope`, `get_team_scope`) but each call does a recursive CTE query. For team scope filtering on every task list request, this could be cached.

---

**Total: 7 items**

Priority order: S1 (template double-encoding), B1 (delete_user NULL reassignment), B3 (board status normalization), then remaining items.
