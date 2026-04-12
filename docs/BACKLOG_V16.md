# BACKLOG v16 — Fresh Codebase Audit

Audit date: 2026-04-12
Codebase: 6400+ LOC backend (55 .rs), 9100+ LOC frontend (53 .ts/.tsx)
Tests: 275 backend, 154 frontend

---

## Confirmed Bugs (18)

- [x] **B1.** `import_tasks_csv` uses manual `BEGIN DEFERRED`/`COMMIT` on pool — may execute on different connections, breaking atomicity. Must use `pool.begin()` for proper transaction.
- [x] **B2.** `import_tasks_json` has no transaction wrapping — partial imports leave orphaned tasks on failure.
- [x] **B3.** `bulk_update_status` doesn't filter `deleted_at IS NULL` — can resurrect soft-deleted tasks.
- [x] **B4.** `delete_user` reassignment subquery `SELECT id FROM users WHERE role='root' LIMIT 1` can return the user being deleted. Add `AND id != ?`.
- [x] **B5.** `get_room_state` project-scoped task query missing `AND t.deleted_at IS NULL` — soft-deleted tasks appear in estimation rooms.
- [x] **B6.** `epic_snapshots` uses `status='completed'` but misses `'done'` status — should use `IN ('completed','done')` like sprint snapshots.
- [x] **B7.** `PRAGMA foreign_keys = ON` only set on one pool connection during migrate — other connections may not enforce FKs. Set via `SqliteConnectOptions.pragma()`.
- [x] **B8.** `cleanup_notifications` datetime format mismatch — `datetime('now','-30 days')` produces `YYYY-MM-DD HH:MM:SS` but `created_at` uses `YYYY-MM-DDTHH:MM:SS.fff`. Use `strftime` with T separator.
- [x] **B9.** `AuditLog` page state not reset when filter changes — user on page 5 changing filter sees empty results.
- [x] **B10.** `EditField` onBlur fires save after Escape — blur event triggers after keydown, saving value user intended to discard.
- [x] **B11.** `i18n.ts` Proxy fallback only applies when `setLocale` is called — initial load with non-English locale gets no fallback for missing keys.
- [x] **B12.** `useSseConnection` debounceTimer never cleared on cleanup — post-unmount `flushChanges` call on unmounted component.
- [x] **B13.** `EpicBurndown` nested `<button>` inside `<button>` — invalid HTML, inconsistent browser behavior.
- [x] **B14.** `Rooms.tsx` room delete has no confirmation dialog — destructive action fires immediately.
- [x] **B15.** `add_sprint_tasks` notification uses wrong event type `"sprint_started"` — should be `"task_added_to_sprint"`.
- [x] **B16.** `accept_estimate` has no value validation — negative or extreme estimates written directly to task.
- [x] **B17.** Webhook URL rewrite to resolved IP breaks HTTPS TLS certificate validation — SNI mismatch.
- [x] **B18.** `auto_archive_days=0` doesn't disable archiving — `.max(1)` clamps to minimum 1 day.

## Security (5)

- [x] **S1.** Webhook secret `derive_key()` falls back to static `"default-key"` when `POMODORO_JWT_SECRET` env var unset — never reads the persisted `.jwt_secret` file. All webhook secrets effectively unencrypted on most installs.
- [x] **S2.** FALSE POSITIVE — `api_limiter` (200 req/60s) defined but never applied to mutation endpoints — only auth endpoints are rate-limited.
- [x] **S3.** FALSE POSITIVE — `delete_user` doesn't revoke active tokens — deleted users retain access for up to 2 hours until JWT expires.
- [x] **S4.** `register` doesn't check for reserved usernames — "admin", "root", "system", "api" can be registered.
- [x] **S5.** `delete_label` has no authorization check — any authenticated user can delete any label.

## Business Logic (8)

- [ ] **BL1.** `update_task` allows setting `parent_id` to a descendant — creates circular reference, causes `get_descendant_ids` to loop until depth limit.
- [ ] **BL2.** `duplicate_task` doesn't copy `work_duration_minutes`, labels, assignees, or dependencies — silently drops task metadata.
- [ ] **BL3.** `room_ws` doesn't re-verify membership after initial check — kicked users keep receiving room state updates until disconnect.
- [ ] **BL4.** `add_time_report` compares username instead of user_id for authorization — username changes between token issuance and request cause false denials.
- [ ] **BL5.** `update_sprint` doesn't validate `capacity_hours` range — `create_sprint` validates 0-10000 but update accepts any value.
- [ ] **BL6.** `create_label` doesn't validate color format — any string accepted. Should validate `#RRGGBB` or `#RGB`.
- [ ] **BL7.** `delete_user` doesn't handle `epic_groups.created_by` — FK violation after user deletion.
- [ ] **BL8.** `get_tasks_full` ETag doesn't reflect soft-delete changes — `deleted_at` update doesn't change `updated_at`, so clients serve stale data.

## UX Improvements (10)

- [ ] **UX1.** `TaskContextMenu` — most action buttons lack `role="menuitem"`, breaking keyboard navigation (ArrowUp/Down only finds items with that role).
- [ ] **UX2.** Template and webhook delete in `SettingsParts.tsx` have no confirmation dialog.
- [ ] **UX3.** Attachment delete in `TaskDetailParts.tsx` has no confirmation dialog.
- [ ] **UX4.** Comment delete in `CommentSection.tsx` has no confirmation dialog.
- [ ] **UX5.** `Recurrence.tsx` pattern names display raw strings ("daily","weekly") — should use existing i18n keys (`t.daily`, `t.weekly`).
- [ ] **UX6.** `CsvImport` drag-and-drop doesn't validate file type — accepts any file.
- [ ] **UX7.** `TrashView` has no "permanently delete" or "empty trash" option.
- [ ] **UX8.** `AuthScreen` server URL edit has no URL format validation.
- [ ] **UX9.** `SprintViews BurnsView` — `taskId` defaults to 0 when tasks empty, submitting burn targets task 0.
- [ ] **UX10.** `ExportButton` submenu uses `group-hover` CSS — disappears when mouse moves between button and submenu.

## Accessibility (10)

- [ ] **A1.** `TaskDetailView` expandable sections (comments/sessions/time/votes) don't use `aria-expanded`.
- [ ] **A2.** `NotificationBell` dropdown has no `role="dialog"`, no `aria-label`, no focus trap, no Escape dismiss.
- [ ] **A3.** `TaskContextMenu` sprint/assignee submenus have no `role="menu"` or `aria-label`.
- [ ] **A4.** `TaskInlineEditors` close buttons use "✕" text with no `aria-label`.
- [ ] **A5.** `TaskDetailParts` attachment download "↓" and delete "✕" buttons have no `aria-label`.
- [ ] **A6.** `AuthScreen` password strength meter has no ARIA live region.
- [ ] **A7.** `AuthScreen` server URL edit button (🌐) has no `aria-label`.
- [ ] **A8.** `TaskActivityFeed` toggle button has no `aria-expanded`.
- [ ] **A9.** `SprintParts` capacity warning "⚠ over capacity" is visual only — no `role="alert"`.
- [ ] **A10.** `TaskList` FTS search results are clickable divs with no `role="button"` or keyboard handler.

## Performance (5)

- [ ] **P1.** `TaskDetailView` N+1 API calls — 4 calls per DetailNode child (time reports, assignees, users, burn-users). 10 children = 40+ calls.
- [ ] **P2.** `useSseConnection` `taskSafety` interval fetches all tasks every 30s even when SSE is working — wasteful for large lists.
- [ ] **P3.** `compare_sprints` loads full task lists just to count completed — should use `COUNT(*)` SQL query.
- [ ] **P4.** `log_burn` loads ALL sprint tasks to check if one task belongs to sprint — should use `SELECT EXISTS(...)`.
- [ ] **P5.** `get_active_timers` does N+1 queries — fetches user and task individually per active timer.

## i18n Gaps (3)

- [ ] **I1.** ~70% of frontend strings still hardcoded English — Timer.tsx, Dashboard.tsx, Sprints.tsx, EstimationRoomView.tsx, History.tsx, Settings.tsx all have significant gaps.
- [ ] **I2.** `Recurrence.tsx` pattern `<option>` values display raw strings instead of using existing i18n keys.
- [ ] **I3.** `TaskContextMenu.tsx` all menu item labels hardcoded English.

## Infrastructure (3)

- [ ] **INF1.** Rate limiter uses `std::sync::Mutex` in async context — blocks tokio thread under high concurrency. Use `parking_lot::Mutex` or `tokio::sync::Mutex`.
- [ ] **INF2.** `list_rooms` and `list_sprints` have hardcoded `LIMIT 200` with no pagination support.
- [ ] **INF3.** `FTS5` search doesn't escape special operators (`NOT`, `OR`, `AND`, `*`, `+`, `-`) — user input interpreted as FTS5 boolean query.

---

**Total: 62 items** (18 bugs, 5 security, 8 business logic, 10 UX, 10 accessibility, 5 performance, 3 i18n, 3 infrastructure)
