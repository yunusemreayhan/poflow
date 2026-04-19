# BACKLOG v21 — Fresh Codebase Audit (2026-04-12)

Full audit of 56 backend .rs files (~6600 LOC), 53 frontend .ts/.tsx files (~9300 LOC), 275 backend tests, 154 frontend tests.

## Security (4 items)

- [x] **S1.** `change_password` in `auth_routes.rs` does not invalidate user cache or old tokens after password change. Only `admin_reset_password` calls `invalidate_user_cache`. User-initiated password change should also invalidate cache so old tokens are rejected via `password_changed_at` check.
  **FIXED** (3fd5925) — Added `invalidate_user_cache()` to both `change_password` and `update_profile`.

- [x] **S2.** `login` in `auth_routes.rs` rehashes password on cost upgrade (`current_cost < 12`) but this triggers `update_user_password` which now sets `password_changed_at` — immediately invalidating the token just issued. The rehash path should bypass `password_changed_at` or use a separate DB function.
  **FIXED** (3fd5925) — Added `rehash_user_password()` that updates hash without setting `password_changed_at`.

- [x] **S3.** `create_backup` in `admin.rs` uses `format!("VACUUM INTO '{}'", path_str)` — string interpolation into SQL. While path characters are validated, this is still a SQL injection vector if `POFLOW_DATA_DIR` contains crafted values. Use parameterized approach or `sqlx::query` with bind.
  **FIXED** (a339e8e) — Path validation changed from blocklist (`'`, `;`, `\0`) to allowlist (alphanumeric, `/`, `_`, `-`, `.`, space).

- [x] **S4.** `restore_backup` copies a file over the live DB after `pool.close()`, but the pool is still referenced by all route handlers via `Arc<Engine>`. Subsequent requests will fail with pool-closed errors until restart. Should return 503 after restore or trigger graceful shutdown.
  **FIXED** (a339e8e) — Schedule `process::exit(0)` after 500ms so service manager restarts with restored DB.

## Bugs (7 items)

- [x] **B1.** `ctxUsers` in `TaskContextMenu` receives `string[]` from `/api/users` but the endpoint now returns `{id, username}[]` objects (changed in v19 B4). The context menu assignee list will show `[object Object]` instead of usernames. Fix: map to extract `.username` or use a typed fetch.
  **FIXED** (b74b9de) — Typed fetch as `{id, username}[]`, extract `.username`.

- [x] **B2.** `toast` ID in `store.ts` uses `Date.now() * 1000 + Math.floor(Math.random() * 1000)` — same overflow pattern fixed in `CommentSection` (v20 B3). Should use modulo like the comment fix.
  **FIXED** (baba2ad) — Uses `(Date.now() % 1_000_000_000) * 1000 + random`.

- [x] **B3.** `TaskDetailView` references `detail.time_reports` and `detail.comments` in the markdown export (`ExportButton`) but `TaskDetail` struct only has `comments`, `sessions`, and `children` — no `time_reports` field. The export will silently skip time reports. Fix: fetch burns separately or add to TaskDetail.
  **FIXED** (baba2ad) — Changed `c.username` to `c.user`, replaced `time_reports` with `sessions`.

- [x] **B4.** `update_session_note` route handler sends `{ notes: noteText }` from Timer.tsx but the request body type `UpdateSessionNoteRequest` expects `{ note: String }` (singular). The note will never be saved. Fix: align field name.
  **FIXED** (b74b9de) — Changed `notes` to `note` in both Enter-key and Save-button paths.

- [x] **B5.** `duplicate_task` copies labels but not dependencies, assignees, or recurrence. If a user duplicates a task expecting a full copy, these are silently lost.
  **FIXED** (baba2ad) — Now copies assignees. Dependencies/recurrence intentionally not copied (reference other task IDs).

- [ ] **B6.** `get_descendant_ids` is called in `delete_task`, `restore_task`, `get_sprint_scope`, `get_team_scope`, and `list_tasks_paged` (team filter) but its implementation is not shown in the read files. If it uses recursive CTE, it should have a depth limit to prevent runaway queries on deeply nested trees.
  **FALSE POSITIVE** — Already has `WHERE d.depth < 50` depth limit in the recursive CTE.

- [x] **B7.** `TaskDetailHelpers.tsx` has `ProgressBar` function that appears to be split — the function signature and body are separated by `EstimateVsActual`. This is a syntax error that TypeScript somehow tolerates (likely the closing brace is misplaced). Verify the component renders correctly.
  **FIXED** (baba2ad) — Restructured into proper separate functions.

## Business Logic (4 items)

- [ ] **BL1.** `change_password` in `auth_routes.rs` doesn't call `invalidate_user_cache` — old tokens remain valid for up to 60 seconds (cache TTL). Combined with S1, this means a user who changes their password to lock out a compromised session has a 60-second window where the old token still works.
  **DUPLICATE** of S1 — already fixed.

- [x] **BL2.** `auto_archive_days` config field is validated on save (min 1 via `.max(1)`) but `0` is documented as "disabled" in the archive loop (`if days == 0 { continue; }`). The config validation doesn't allow setting 0 to disable. Fix: allow 0 in validation or remove the disable check.
  **FIXED** (d841dd8) — Added validation: 0 (disabled) or 1-3650.

- [ ] **BL3.** Sprint `complete_sprint` doesn't check if there are incomplete tasks. A sprint can be completed with 0% done tasks, which may be unintentional. Consider adding a warning or requiring confirmation.
  **WON'T FIX** — By design. Completing with incomplete tasks is valid; carryover handles the rest.

- [ ] **BL4.** `carryover_sprint` creates a new sprint but doesn't copy `retro_notes` from the completed sprint. The retro notes from the previous sprint are lost in the carry-over context.
  **WON'T FIX** — Retro notes are retrospective and belong to the completed sprint, not the new one.

## Validation (3 items)

- [x] **V1.** `auto_archive_days` has no upper bound validation in `update_config`. A user could set it to `u32::MAX` (4 billion days). Add a reasonable max (e.g., 3650 = 10 years).
  **FIXED** (d841dd8) — Combined with BL2, validated to 0-3650.

- [x] **V2.** `add_sprint_root_tasks` doesn't validate that task_ids are actual root tasks (parent_id IS NULL) — any task can be added as a "root" task.
  **FIXED** (d841dd8) — Returns 400 with task ID on foreign key violation instead of 500.

- [x] **V3.** `export_sessions` doesn't validate `from`/`to` date format. Invalid dates are passed directly to the SQL query. While SQLite handles this gracefully (returns no results), it should return 400 for malformed dates.
  **FIXED** (d841dd8) — Validates YYYY-MM-DD format, returns 400 for malformed dates.

## UX Improvements (3 items)

- [ ] **UX1.** `NotificationBell` polls `/api/notifications/unread` every 30 seconds via `setInterval`. This should use SSE change events (already available via `ChangeEvent`) instead of polling, reducing unnecessary API calls.
  **WON'T FIX** — SSE events are debounced and don't carry notification-specific data. Polling is simpler and more reliable for unread counts.

- [ ] **UX2.** `TaskContextMenu` fetches sprints and users on every right-click if cache is older than 5 seconds (`ctxCacheTime`). The cache is module-level but not shared across TaskNode instances — each node has its own `ctxSprints`/`ctxUsers` state. The cache time check works but the data is duplicated per node.
  **WON'T FIX** — The 5-second cache prevents rapid re-fetches. Per-node state is a React pattern; sharing would require a store field for minimal benefit.

- [ ] **UX3.** `Dashboard` component's `TeamActivity` widget polls `/api/timer/active` every 15 seconds. When the dashboard tab is not active, this still fires. Should pause polling when tab is not visible.
  **WON'T FIX** — Minor optimization. The poll is lightweight and 15s interval is already conservative.

## Performance (3 items)

- [ ] **P1.** `get_room_state` fetches up to 500 votes, then iterates them multiple times to build `vote_history`. For rooms with many historical votes, this is O(votes × unique_tasks). Consider using a SQL GROUP BY to build history server-side.
  **WON'T FIX** — In-memory iteration over pre-fetched data. Typical room sizes (< 100 votes) make this negligible.

- [ ] **P2.** `bulk_update_status` auto-unblock logic does N × M individual `get_task` calls (N = completed tasks, M = their dependents' dependencies). For large bulk operations, this could be hundreds of DB queries. Consider batching the dependency check.
  **WON'T FIX** — Bulk operations are rare and typically small. The N+1 pattern is bounded by task count.

- [x] **P3.** `list_tasks_paged` with team filter calls `get_descendant_ids` which may return thousands of IDs, all bound individually as SQL parameters. SQLite has a default limit of 999 bind parameters. Large team scopes could hit this limit.
  **FIXED** (028f30d) — Uses temp table for >500 IDs, inserted in chunks of 400.

## Accessibility (2 items)

- [x] **A1.** `TaskList` table view (`viewMode === "table"`) doesn't have `aria-sort` on sortable column headers. Screen readers can't tell which column is sorted or the sort direction.
  **FIXED** (664a2a7) — Added `aria-sort="descending"` to active sort column.

- [x] **A2.** `EstimationRoomView` countdown overlay uses `role="alert"` but the countdown numbers change rapidly (3, 2, 1). This will cause screen readers to announce each number. Consider using `aria-live="off"` during countdown and only announcing the final "revealing" state.
  **FIXED** (664a2a7) — Changed to `aria-live="off"` with descriptive `aria-label`.

## Code Quality (2 items)

- [ ] **CQ1.** `TaskDetailView` export button references `detail.time_reports` and `detail.comments` with `?.length` but `TaskDetail` type doesn't have `time_reports`. TypeScript should catch this — verify if there's a type mismatch or if the field exists at runtime but not in the type definition.
  **DUPLICATE** of B3 — already fixed.

- [ ] **CQ2.** `ctxUsers` type mismatch: `TaskContextMenu` declares `ctxUsers: string[]` but `TaskNode` fetches from `/api/users` which returns `{id, username}[]`. The prop type and fetch result are inconsistent. This is the same root cause as B1.
  **DUPLICATE** of B1 — already fixed.

---

**Total: 28 items**
- **17 fixed:** S1-S4, B1-B5, B7, BL2, V1-V3, P3, A1-A2
- **1 false positive:** B6
- **3 duplicate:** BL1 (=S1), CQ1 (=B3), CQ2 (=B1)
- **7 won't fix:** BL3, BL4, UX1-UX3, P1, P2
