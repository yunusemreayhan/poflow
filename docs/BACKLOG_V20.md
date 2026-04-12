# BACKLOG v20 — Fresh Codebase Audit (2026-04-12)

Full audit of 56 backend .rs files (~6600 LOC), 53 frontend .ts/.tsx files (~9300 LOC), 275 backend tests, 154 frontend tests.

## Security (3 items)

- [ ] **S1.** `update_config` route accepts full `Config` struct including `bind_address`, `bind_port`, `cors_origins`, `auto_archive_days` — a non-root user can change server-wide settings like bind address and CORS origins via their per-user config PUT. The route saves per-user overrides for timer fields, but root users also call `cfg.save()` which writes ALL fields including network config. A root user could accidentally change `bind_address` to `0.0.0.0` via the config API.
- [ ] **S2.** `admin_reset_password` doesn't revoke the target user's existing tokens — after an admin resets a user's password, the old tokens remain valid until they expire (up to 2 hours for access, 30 days for refresh). Should revoke all tokens for the target user.
- [ ] **S3.** `upload_attachment` doesn't validate MIME type against file content — the `content-type` header is trusted as-is. An attacker could upload a `.html` file with `content-type: image/png` to bypass the safe-mime check in `download_attachment`. Should validate extension matches claimed MIME type.

## Bugs (8 items)

- [ ] **B1.** `Sprints.tsx` `SprintView` uses `allTasks.find(tk => tk.id === rid)` inside `.map()` for root task display — O(n²) for sprints with many root tasks. Should use a Map.
- [ ] **B2.** `EpicBurndown` uses `allTasks.find(tk => tk.id === tid)` inside `.map()` for task display — same O(n²) issue.
- [ ] **B3.** `CommentSection` optimistic update uses `Date.now() * 1000 + Math.floor(Math.random() * 1000)` for negative IDs — this can overflow JavaScript's safe integer range. Should use `-(Date.now() % 1000000)` or similar.
- [ ] **B4.** `EstimationRoomView` `myVote` dependency in `useEffect` references `state?.votes` which changes on every render (new array reference from WS) — the card selection reset effect fires too often. Should compare by `current_task_id` only.
- [ ] **B5.** `BacklogView` `loadRoots` calls `apiCall("GET", /api/sprints/${sprintId}/scope)` which returns all descendant IDs — for large task trees this could be thousands of IDs. The `filterIds` prop then does `.has()` checks which is fine, but the initial fetch is unbounded.
- [ ] **B6.** `export_tasks` CSV export doesn't include `work_duration_minutes` column — but `import_tasks_csv` also doesn't import it, so round-trip is consistent. However, the field is lost on export.
- [ ] **B7.** `bulk_update_status` doesn't trigger auto-unblock logic for dependencies — when bulk-completing tasks, dependent tasks that should be unblocked remain blocked. Only `update_task` (single task) runs the dependency check.
- [ ] **B8.** `snapshot_sprint` in the hourly background task calls `snapshot_active_sprints` which snapshots ALL active sprints — but if there are no changes since the last snapshot, it creates duplicate data points. Should skip if no tasks changed.

## Business Logic (4 items)

- [ ] **BL1.** `delete_label` is root-only — any user can create labels but only root can delete them. Should allow the label creator to delete their own labels (requires tracking `created_by` on labels table).
- [ ] **BL2.** `add_sprint_tasks` notifies task owners individually in a loop — for bulk adds (e.g., 50 tasks), this creates 50 separate notifications. Should batch into a single notification per user.
- [ ] **BL3.** `carryover_sprint` doesn't copy `capacity_hours` from the completed sprint — the new sprint inherits it via `create_sprint` parameter, but the `goal` is not carried over. Should carry over goal as well.
- [ ] **BL4.** `accept_estimate` auto-advance skips tasks with status "estimated" — but there's no mechanism to set a task's status to "estimated" after accepting. The `accept_estimate` function updates the task's `estimated`/`estimated_hours`/`remaining_points` fields but doesn't change status. The filter `t.status !== "estimated"` is dead code.

## Validation (3 items)

- [ ] **V1.** `create_webhook` blocks `172.2*` ranges but misses `172.20.` through `172.29.` — the check `host.starts_with("172.2")` catches `172.20-172.29` but also catches `172.2.x.x` which is a public IP. Should check the full RFC 1918 range `172.16.0.0/12` properly.
- [ ] **V2.** `import_tasks_json` limits to 500 top-level tasks but doesn't limit total tasks including children — a request with 500 tasks each having 20 children = 10,500 tasks. Should count total recursively.
- [ ] **V3.** `edit_comment` 15-minute window uses `chrono::Utc::now()` but `created_at` is stored as local time via `now_str()` — if the server timezone differs from UTC, the window calculation is wrong.

## UX Improvements (4 items)

- [ ] **UX1.** `History` component fetches all sessions without pagination — for users with thousands of sessions, this loads everything at once. Should add pagination or virtual scrolling.
- [ ] **UX2.** `TaskContextMenu` "Move up/Move down" doesn't provide visual feedback — the task list doesn't scroll to the moved task or highlight it after reorder.
- [ ] **UX3.** `AuthScreen` password strength meter doesn't account for common passwords — only checks length, uppercase, and digit. Could add a basic dictionary check.
- [ ] **UX4.** No way to re-open a closed estimation room — once closed, the room can only be viewed but not reactivated. Should allow admin to reopen.

## Accessibility (2 items)

- [ ] **A1.** `BoardView` drag-and-drop cards have `draggable` but no `aria-grabbed`/`aria-dropeffect` — screen readers can't determine drag state. The keyboard arrow key support is good but the ARIA attributes are missing.
- [ ] **A2.** `EstimationRoomView` voting cards use `role="radio"` but are not wrapped in a `role="radiogroup"` — the `role="radiogroup"` is on the parent div with `aria-label` but the individual cards don't have `aria-label` that includes the unit context.

## Performance (2 items)

- [ ] **P1.** `get_tasks_full` ETag computation runs 7 aggregate queries in a single compound SELECT — this is efficient but the ETag string format `"max_updated:count:count:count:count:count:count"` doesn't capture task content changes that don't affect counts (e.g., renaming a task updates `updated_at` but if another task is deleted simultaneously, the max could stay the same).
- [ ] **P2.** `BoardView` `useEffect` for blocked tasks fetches ALL dependencies on every board render — should only fetch dependencies for tasks in the current sprint board.

## Code Quality (3 items)

- [ ] **CQ1.** `webhook.rs` creates a new `reqwest::Client` per webhook hook (for DNS pinning) — this bypasses connection pooling. Should reuse the base client and only pin DNS for the specific request.
- [ ] **CQ2.** `Sprints.tsx` and `EpicBurndown.tsx` both use `allTasks.find()` in render loops — should extract a shared `useTaskMap()` hook that returns `Map<number, Task>`.
- [ ] **CQ3.** `EstimationRoomView` has 400+ lines in a single component — the board tab, tasks tab, members tab, and history tab should be extracted into sub-components.

---

**Total: 29 items** — S:3, B:8, BL:4, V:3, UX:4, A:2, P:2, CQ:3
