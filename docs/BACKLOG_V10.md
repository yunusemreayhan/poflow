# BACKLOG v10 — pojidora

Generated from full codebase analysis (5485 LOC backend, 8244 LOC frontend, 211 backend tests, 154 frontend tests).

---

## Bugs (B1–B10)

**B1: `backup` endpoint SQL injection via path traversal**
`admin.rs:create_backup` uses `format!("VACUUM INTO '{}'", backup_path.display())` — if the data dir contains a single quote, the SQL breaks. Use parameterized approach or sanitize path.

**B2: `useRoomWebSocket` cleanup race condition**
The `connect()` async function returns a cleanup function, but if the component unmounts before `connect()` resolves, `cleanup` is undefined and the WebSocket leaks. The `useEffect` cleanup runs but `cleanup?.()` is a no-op.

**B3: Timer `onKeyDown` duplicate handler on TaskNode**
`TaskNode.tsx` has two `onKeyDown` props on the same `motion.div` — the second one (with `e.altKey` reorder logic) silently overrides the first. The alt+arrow reorder shortcuts are dead code.

**B4: Monthly recurrence day clamping off-by-one**
`main.rs` monthly recurrence: `NaiveDate::from_ymd_opt(y, m + if m < 12 { 1 } else { 0 }, 1)` — when `m=12`, this computes `from_ymd(y+1, 12, 1)` instead of `from_ymd(y+1, 1, 1)`. The `pred_opt()` then gives Nov 30 instead of Dec 31.

**B5: SSE timer broadcasts wrong user's state**
`sse_timer` in `misc.rs`: the `timer_rx.changed()` branch checks `state.current_user_id == user_id || state.current_user_id == 0`, but the watch channel only holds the *last* user's state. If user A ticks, user B's SSE gets user A's state (filtered by the check, but `current_user_id == 0` leaks idle states). The `engine.get_state(user_id)` re-fetch mitigates this but adds unnecessary DB load.

**B6: `ExportButton` in `TaskDetailHelpers.tsx` simplified export lost multi-format support**
The Q2 refactor replaced the original 3-format export (md/json/xml) with a single markdown export. The original `TaskDetailView` had a format picker dropdown.

**B7: Notification prefs not checked before dispatching webhooks/toasts**
`F12` added notification_prefs table but the backend doesn't query it before sending desktop notifications for task_assigned, comment_added, etc. Only timer completion checks `notify_desktop`.

**B8: `get_tasks_full` ETag doesn't include notification_prefs or labels changes**
The ETag hash uses `tasks`, `sprint_tasks`, `burn_log`, `task_assignees` counts — but label changes and attachment changes don't invalidate it, causing stale data.

**B9: `accept_estimate` auto-advance skips tasks with children**
`rooms.rs:accept_estimate` filters `!has_children.contains(&t.id)` for auto-advance, but `has_children` is built from `parent_id` of all tasks — not just room tasks. A task that's a parent in the global tree but a leaf in the room scope gets skipped.

**B10: Webhook retry uses `try_clone()` which fails for streamed bodies**
`webhook.rs` retry loop: `req.try_clone().unwrap_or_else(|| client.post(&hook.url).body(body_str.clone()))` — the fallback loses all headers (content-type, x-pomodoro-event, x-pomodoro-signature). Should rebuild the full request.

---

## Security (S1–S5)

**S1: Backup endpoint path injection**
`create_backup` constructs SQL with `format!()` — should use a safe path or parameterized query. Also, backup files are world-readable by default (no chmod 600).

**S2: User cache never pruned**
`auth.rs:USER_CACHE` grows unboundedly — entries are inserted but never removed (except on explicit `invalidate_user_cache`). Long-running servers accumulate stale entries.

**S3: Webhook dispatch leaks internal error details**
`webhook.rs` logs full error messages including potentially sensitive DB errors. Should sanitize before logging.

**S4: CSRF token not validated on WebSocket upgrade**
`room_ws` uses ticket auth but doesn't check `x-requested-with` header. WebSocket upgrades bypass the CSRF middleware since they're GET requests.

**S5: Rate limiter uses `std::sync::Mutex` (blocking)**
`RateLimiter.attempts` uses `std::sync::Mutex` which blocks the async runtime. Should use `tokio::sync::Mutex` or `parking_lot::Mutex`.

---

## Validation (V1–V5)

**V1: `LogBurnRequest` allows negative hours/points**
`burns.rs:log_burn` doesn't validate that `hours` and `points` are non-negative.

**V2: Comment content length unlimited**
`add_comment` in `comments.rs` has no length limit on `content`. Should cap at 10000 chars.

**V3: Time report hours unbounded**
`add_time_report` in `burns_task.rs` doesn't cap `hours` — a user could log 999999 hours.

**V4: Sprint date validation: end_date before start_date**
`create_sprint` and `update_sprint` validate date format but don't check that `end_date >= start_date`.

**V5: Webhook URL length unlimited**
`create_webhook` doesn't limit URL length. Should cap at 2000 chars.

---

## Performance (P1–P4)

**P1: `get_task_detail` N+1 query for children**
`db/mod.rs:get_task_detail` recursively calls itself for each child task, creating N+1 queries. Should use a CTE to fetch the entire subtree in one query.

**P2: `get_room_state` fetches all room tasks on every WebSocket broadcast**
Every `ChangeEvent::Rooms` triggers a full `get_room_state` query per connected WebSocket client. Should debounce or cache.

**P3: `list_tasks_paged` with team scope does recursive CTE per request**
`get_descendant_ids` runs a recursive CTE every time tasks are listed with a team filter. Should cache team scope for a few seconds.

**P4: `get_history` loads all ancestors via CTE for every session**
The ancestor CTE in `get_history` runs for every unique task_id in the result set. For large histories, this is expensive.

---

## Features (F1–F12)

**F1: Task archiving with auto-cleanup**
Add `archived` status that hides tasks from default views. Auto-archive completed tasks older than 90 days. Add "Archive" button to context menu.

**F2: Sprint capacity planning**
Show team capacity (total available hours) vs. committed hours on sprint board. Add `capacity_hours` field to sprints.

**F3: Task time estimate vs. actual comparison chart**
Add a chart to task detail showing estimated vs. actual hours over time. Use existing burn_log data.

**F4: Bulk task import from JSON**
Currently only CSV import exists. Add JSON import for structured task trees (with parent-child relationships).

**F5: Sprint carry-over**
When completing a sprint, offer to move incomplete tasks to a new sprint automatically.

**F6: Task watchers (subscribe to updates)**
Allow users to "watch" tasks they don't own. Watched tasks appear in a filtered view and trigger notifications on changes.

**F7: Session notes inline editing**
Currently session notes are read-only after creation. Allow editing notes on completed sessions.

**F8: Room estimation history export**
Export all estimation results from a room as CSV/JSON for retrospective analysis.

**F9: Configurable work session duration per task**
Allow overriding the default work duration for specific tasks (e.g., 45min for deep work tasks).

**F10: Dashboard/overview tab**
Add a dashboard showing: today's focus time, active sprint progress, upcoming due dates, recent activity. Replace or augment the history tab.

**F11: Task dependency blocking**
When a task has unresolved dependencies, prevent starting a timer on it. Show a warning in the UI.

**F12: Attachment drag-and-drop upload**
Allow dragging files onto the task detail view to upload attachments.

---

## Tests (T1–T8)

**T1: Backup endpoint test**
Test `POST /api/admin/backup` — verify file created, size returned, non-root rejected.

**T2: Notification preferences CRUD test**
Test `GET/PUT /api/profile/notifications` — default values, toggle, unknown event type rejected.

**T3: Bulk status update test**
Test `PUT /api/tasks/bulk-status` — ownership check, invalid status rejected, empty list no-op.

**T4: Task restore from trash test**
Test `POST /api/tasks/{id}/restore` — verify task and descendants restored, non-owner rejected.

**T5: Sprint carry-over / completion test**
Test `POST /api/sprints/{id}/complete` — verify snapshot taken, status changed, incomplete tasks preserved.

**T6: Refresh token rotation test**
Test `POST /api/auth/refresh` — verify old refresh token revoked, new tokens issued, expired token rejected.

**T7: WebSocket room state broadcast test**
Test that WebSocket clients receive state updates when votes are cast and revealed.

**T8: Concurrent task update conflict detection test**
Test `expected_updated_at` conflict detection — verify 409 returned when task modified by another user.

---

## UX (U1–U7)

**U1: Timer shows task progress (pomodoros completed / estimated)**
Show "3/8 🍅" on the timer ring when a task is selected, so users know how far along they are.

**U2: Sprint board column WIP limits**
Allow setting max tasks per column (e.g., max 3 in "In Progress"). Visual warning when exceeded.

**U3: Task list column view option**
Add a toggle between tree view and flat column/table view for tasks, showing all fields in a sortable table.

**U4: Keyboard shortcut for quick task creation**
Press `n` on tasks tab to focus the new task input (partially exists but `data-new-task-input` attribute is missing from the input element).

**U5: Toast notification stacking limit**
No limit on simultaneous toasts. If many tasks change at once (bulk update), the screen fills with toasts. Cap at 3 visible.

**U6: Sprint burndown chart date axis formatting**
Burndown chart X-axis shows raw ISO dates. Should show "Mon", "Tue" or "Apr 12" format.

**U7: Estimation room card selection feedback**
When a user selects a card in the estimation room, there's no haptic/visual confirmation beyond the card highlight. Add a brief scale animation.

---

## Code Quality (Q1–Q5)

**Q1: Consolidate `map_err(internal)` calls using `From<sqlx::Error>`**
Q6 from v9 added `From<sqlx::Error>` but 176 existing call sites still use `.map_err(internal)`. Migrate them to use `?` directly.

**Q2: Extract `escape_csv` and `parse_csv_line` to shared utility**
These functions in `export.rs` are duplicated logic. Move to a `csv_utils.rs` module.

**Q3: Remove unused imports across frontend**
Several components import icons/hooks that are no longer used after v9 refactors (e.g., `TaskList.tsx` still imports `React` but doesn't use JSX transform requiring it).

**Q4: Type-safe API response handling**
`apiCall` returns `T` but many callers don't handle the case where the response shape doesn't match. Add runtime validation or at least null checks.

**Q5: Deduplicate sprint ownership checks**
`sprints.rs` repeats the pattern `get_sprint → check is_owner_or_root → return 403` in 8 endpoints. Extract to a helper function.

---

## Documentation (D1–D3)

**D1: Add ARCHITECTURE.md**
Document the overall system architecture: backend modules, frontend component tree, data flow, WebSocket/SSE protocol, auth flow.

**D2: Add ENV_VARS.md**
Document all environment variables: `POMODORO_JWT_SECRET`, `POMODORO_CORS_ORIGINS`, `POMODORO_LOG_JSON`, `POMODORO_SWAGGER`, `POMODORO_ROOT_PASSWORD`, `ACCESS_TOKEN_EXPIRY_SECS`, `REFRESH_TOKEN_EXPIRY_SECS`.

**D3: Inline API endpoint documentation**
Add brief doc comments to all route handler functions explaining purpose, auth requirements, and notable behavior.

---

## DevOps (O1–O3)

**O1: Database migration version tracking in health endpoint**
`/api/health` should report the current schema migration version so operators can verify migrations ran.

**O2: Structured error logging with request context**
Error logs don't include request ID, user ID, or endpoint path. Add a middleware that attaches a request ID and logs it on errors.

**O3: Configurable backup retention**
`create_backup` creates backups but never cleans old ones. Add a retention policy (e.g., keep last 10 backups, delete older).

---

## Accessibility (A1–A3)

**A1: Timer ring SVG needs better screen reader support**
The SVG `role="progressbar"` is good but the `aria-label` uses interpolated values that may not update live. Add `aria-live="polite"` to the parent.

**A2: Estimation room card grid needs keyboard navigation**
The point/hour cards in the estimation room are buttons but lack arrow-key navigation between them. Add `role="radiogroup"` pattern.

**A3: Toast notifications need `role="alert"` for errors**
Error toasts use `role="alert"` but success toasts don't have `role="status"`. The container has `aria-live="polite"` which is correct, but individual error toasts should use `role="alert"` for immediate announcement.

---

## Cleanup (C1–C3)

**C1: Remove dead `FolderOpen` import in `TaskList.tsx`**
After Q1 split, `TaskList.tsx` imports `FolderOpen` from lucide but it's only used in the inline add-root section. Verify it's still needed.

**C2: Consolidate `now_str()` usage**
`now_str()` is called in many places. Some routes also call `chrono::Utc::now()` separately. Standardize on `now_str()` everywhere.

**C3: Remove `CommentSection` import from `TaskDetailView.tsx`**
After Q2 refactor, check if `CommentSection` is still directly imported or if it's only used through `DetailNode`.

---

**Total: 65 items** (10 bugs, 5 security, 5 validation, 4 performance, 12 features, 8 tests, 7 UX, 5 code quality, 3 documentation, 3 devops, 3 accessibility, 3 cleanup)

**Suggested priority order:**
1. Bugs B1–B10 (especially B1 SQL injection, B3 dead code, B4 recurrence)
2. Security S1–S5
3. Validation V1–V5
4. Tests T1–T8
5. Performance P1–P4
6. Features F1–F12
7. UX U1–U7
8. Code Quality Q1–Q5
9. Documentation D1–D3
10. DevOps O1–O3
11. Accessibility A1–A3
12. Cleanup C1–C3
