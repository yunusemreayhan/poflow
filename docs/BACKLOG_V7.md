# Backlog v7 — pojidora

Generated: 2026-04-12
Backend: 174 tests | Frontend: 134 tests | TS strict: clean

---

## Security (S1–S10)

- **S1** — `attachments.rs`: Random key uses `/dev/urandom` via `std::fs::File::open` — not portable and panics on failure. Use `rand` crate or `getrandom` for cross-platform CSPRNG.
- **S2** — `attachments.rs:42`: Filename sanitization allows `..` sequences after filtering (e.g. `..file` becomes `..file`). Should reject or strip `..` explicitly.
- **S3** — `auth.rs`: In-memory blocklist grows unbounded — no periodic cleanup of expired entries from the `HashSet`. Only DB is pruned on `revoke_token`.
- **S4** — `auth.rs:76`: `create_token` uses 2-hour expiry hardcoded. Should be configurable via env var or config.
- **S5** — `routes/history.rs`: `get_history` accepts `user_id` query param but doesn't verify the caller has permission to view other users' history. Non-root users can see anyone's sessions.
- **S6** — `routes/labels.rs`: `create_label`, `delete_label`, `add_task_label`, `remove_task_label` have no authorization — any authenticated user can manage any label/task-label.
- **S7** — `routes/dependencies.rs`: `add_dependency`, `remove_dependency` have no ownership check — any user can modify any task's dependencies.
- **S8** — `routes/recurrence.rs`: `set_recurrence`, `remove_recurrence` have no ownership check — any user can set recurrence on any task.
- **S9** — `routes/templates.rs`: `delete_template` has no ownership check — any user can delete any template.
- **S10** — `webhook.rs`: Webhook dispatch retries use `try_clone()` which may fail silently, falling back to a bare POST without auth headers.

## Bugs (B1–B12)

- **B1** — `engine.rs:tick()`: Drops and re-acquires `states` lock between config fetch and timer advancement. Another request could modify state in between, causing a missed tick or double-completion.
- **B2** — `engine.rs:skip()`: Doesn't end the session as "completed" — uses "skipped" status. But `get_today_completed_for_user` only counts "completed" sessions, so skipped work sessions don't count toward daily goal.
- **B3** — `routes/rooms.rs:cast_vote`: Checks room membership but doesn't verify `room.status == "voting"`. Users can vote when room is in "lobby" or "revealed" state.
- **B4** — `routes/sprints.rs:update_sprint`: Validates `req.status.is_some()` to reject status changes, but then passes `req.status.as_deref()` to `db::update_sprint` which would be `None` — harmless but confusing dead code path.
- **B5** — `store.ts:loadStats`: Swallows all errors silently (`catch { /* ignore */ }`). If stats API fails, loading spinner never clears because `loading.stats` stays true.
- **B6** — `store.ts:loadHistory`: Same issue — error swallowed, `loading.history` stays true on failure.
- **B7** — `History.tsx`: `filteredHistory` depends on `userFilter` but `userFilter` filters by `s.user` which is the display username. If username changes (via profile update), filter breaks.
- **B8** — `EstimationRoomView.tsx:38`: `myVote` dependency in useEffect uses `myVote?.value` but `myVote` is derived from `state` which changes on every SSE push, causing unnecessary card resets.
- **B9** — `TaskList.tsx`: Long-press handler (`longPressRef`) doesn't call `e.preventDefault()` on touchstart, so the browser may also trigger a context menu or text selection.
- **B10** — `Sprints.tsx:Column`: `onDragOver` adds class directly via `classList.add` — bypasses React's virtual DOM, may cause stale class state.
- **B11** — `CommentSection.tsx`: Delete button has no ownership check in UI — shows delete for all comments, but backend rejects non-owners. Should hide button for non-owned comments.
- **B12** — `Settings.tsx`: `isDirty` comparison uses `JSON.stringify` which is order-dependent. If config fields arrive in different order from server, it shows false dirty state.

## Validation (V1–V8)

- **V1** — `routes/burns.rs:log_burn`: No validation that sprint exists or is active before logging burn.
- **V2** — `routes/rooms.rs:create_room`: No limit on number of rooms a user can create (potential abuse).
- **V3** — `routes/teams.rs:add_team_root_tasks`: N+1 queries — calls `get_task` and `add_team_root_task` in a loop. Should batch validate.
- **V4** — `routes/epics.rs:add_epic_group_tasks`: Same N+1 pattern — loops over task_ids with individual queries.
- **V5** — `routes/recurrence.rs:set_recurrence`: No validation on `next_due` format (should be YYYY-MM-DD).
- **V6** — `routes/webhooks.rs:create_webhook`: `events` field accepts any string — should validate against known event names.
- **V7** — `routes/comments.rs:add_comment`: No validation that the referenced `session_id` exists or belongs to the task.
- **V8** — `routes/attachments.rs`: No validation that `task_id` exists before uploading attachment.

## Performance (P1–P10)

- **P1** — `routes/misc.rs:get_tasks_full`: ETag query runs 5 subqueries. Should use a single query or materialized counter.
- **P2** — `routes/misc.rs:get_tasks_full`: `tokio::join!` fetches 4 datasets but `list_tasks` doesn't use the same filter as the paginated endpoint — always fetches ALL tasks.
- **P3** — `engine.rs:tick()`: Drops and re-acquires `states` lock for every tick cycle. With many users, this creates lock contention. Consider per-user locks or lock-free approach.
- **P4** — `store.ts:loadTasks`: `tasksChanged` comparison iterates all tasks on every load. With 1000+ tasks, this is O(n) on every SSE push.
- **P5** — `History.tsx`: `heatmapData` computes 365 days of data on every render. Should memoize with `useMemo` (currently is memoized but depends on `filteredHistory` which changes on every `userFilter` change).
- **P6** — `TaskDetailView.tsx`: Fetches `/api/tasks/{id}/time`, `/api/tasks/{id}/assignees`, `/api/tasks/{id}/burn-users` on every mount. These could be cached or batched into a single endpoint.
- **P7** — `Sprints.tsx:SprintView`: `load()` fetches detail + board sequentially. Board fetch could be parallelized.
- **P8** — `db/tasks.rs:get_descendant_ids`: Recursive CTE with no depth limit. Deeply nested trees could cause slow queries.
- **P9** — `webhook.rs:dispatch`: Resolves DNS for every webhook delivery. Should cache DNS results briefly.
- **P10** — `App.tsx`: SSE reconnect creates new `EventSource` on every reconnect without cleaning up the old one's event listeners first.

## Code Quality (Q1–Q12)

- **Q1** — `routes/mod.rs`: `RateLimiter` uses `std::sync::Mutex` inside async context. Should use `tokio::sync::Mutex` or `parking_lot::Mutex`.
- **Q2** — `routes/tasks.rs:bulk_update_status`: Builds SQL with string formatting (`format!`). While bind params are used for values, the placeholder count is computed from user input length — should cap `task_ids` length.
- **Q3** — `engine.rs`: `Engine` struct has all `pub` fields. Internal state (`states`, `config`, `tx`, `changes`) should be private with accessor methods.
- **Q4** — `store.ts`: `_tasksLoadedAt` is a module-level `let` export — breaks encapsulation. Should be inside the store.
- **Q5** — `TaskList.tsx`: `TaskNode` is ~470 lines. Should decompose into `TaskRow`, `TaskActions`, `TaskDragHandler` sub-components.
- **Q6** — `App.tsx`: SSE connection logic is ~100 lines inline in a `useEffect`. Should extract to a custom hook `useSseConnection`.
- **Q7** — `Sprints.tsx`: `SprintView` and `Column` are defined inside the file but not memoized. `Column` re-renders on every parent render.
- **Q8** — `routes/types.rs`: Request/response types are in a single file. Should split by domain (auth types, task types, sprint types).
- **Q9** — `db/mod.rs:migrate()`: All migrations in a single function with 30+ `CREATE TABLE` statements. Hard to track schema changes.
- **Q10** — `i18n.ts`: Turkish translations are inline in the same file. Should split locales into separate files for maintainability.
- **Q11** — `store/types.ts`: Some interfaces use compact single-line format, others use multi-line. Inconsistent formatting.
- **Q12** — `routes/rooms.rs:room_ws`: WebSocket handler is 50+ lines in the rooms route file. Should extract to its own module.

## Features (F1–F15)

- **F1** — No task archiving — deleted tasks are gone forever. Add soft-delete with "archived" status and restore capability.
- **F2** — No task search across all fields — current search only matches title/project/user/tags. Add full-text search including description and comments.
- **F3** — No task templates — users must manually create recurring task structures. Add "create from template" using the existing templates API.
- **F4** — No sprint retrospective view — retro_notes exist but there's no dedicated UI for sprint retrospectives with structured feedback.
- **F5** — No task time tracking visualization — burn data exists but there's no per-task time chart showing hours over time.
- **F6** — No notification preferences UI — backend supports per-user `notify_desktop` and `notify_sound` but Settings.tsx doesn't expose them.
- **F7** — No task due date reminders in UI — backend sends desktop notifications but the frontend has no visual indicator for upcoming/overdue tasks.
- **F8** — No bulk task import from UI — backend has CSV import endpoint but no frontend UI for it.
- **F9** — No task activity feed — audit log exists but there's no per-task activity timeline showing who changed what.
- **F10** — No estimation room task filtering — room shows all tasks but can't filter by project/status/assignee.
- **F11** — No sprint velocity chart in UI — backend has `/api/sprints/velocity` endpoint but no frontend visualization.
- **F12** — No dark/light theme toggle persistence — theme is in config but the toggle in the sidebar doesn't sync with per-user config.
- **F13** — No WebSocket reconnect for estimation rooms — if WS drops, room state goes stale until manual refresh.
- **F14** — No task sorting options in UI — tasks can only be reordered by drag. Add sort by priority/due date/created date.
- **F15** — No export for sprint burndown data — burndown chart data can't be downloaded as CSV.

## UX (U1–U10)

- **U1** — `AuthScreen.tsx`: No password strength indicator. Users get a generic "min 6 chars" hint but no visual feedback.
- **U2** — `AuthScreen.tsx`: "Already have an account? Sign in" / "No account? Register" strings are not i18n'd.
- **U3** — `Rooms.tsx`: No loading state when fetching rooms list. Screen is blank until data arrives.
- **U4** — `EpicBurndown.tsx`: No loading state when fetching epic detail. Chart area is empty during load.
- **U5** — `TaskDetailView.tsx`: Back button returns to task list root. With deep navigation stack, should show breadcrumb trail.
- **U6** — `Sprints.tsx`: Sprint create form has no date picker — raw text input for dates. Should use `<input type="date">`.
- **U7** — `Settings.tsx`: User management section shows all users in a flat list. No search/filter for large user bases.
- **U8** — `Timer.tsx`: No visual indication of which task is currently being timed (task name not shown on timer).
- **U9** — `CommentSection.tsx`: No optimistic update — comment appears only after server round-trip.
- **U10** — `TaskContextMenu.tsx`: Context menu doesn't show current status/priority — user must guess current values.

## Accessibility (A1–A8)

- **A1** — `Rooms.tsx`: Room cards have no keyboard activation — only `onClick`, no `onKeyDown` for Enter/Space.
- **A2** — `EpicBurndown.tsx`: Area chart has no accessible data table fallback for screen readers.
- **A3** — `SprintViews.tsx`: Burndown/velocity charts have no accessible data table fallback.
- **A4** — `AuthScreen.tsx`: Password show/hide toggle uses emoji (🙈/👁) — screen readers may not convey meaning. Use text or proper icon.
- **A5** — `Select.tsx`: Custom select doesn't announce selected value change to screen readers (missing `aria-activedescendant`).
- **A6** — `CommentSection.tsx`: Delete button has no `aria-label` — screen reader just says "button".
- **A7** — `AuditLog.tsx`: Filter dropdown has `aria-label` but results table has no `role="table"` or row semantics.
- **A8** — `Recurrence.tsx`: Pattern select and date input have no visible labels — only placeholder text.

## Tests (T1–T12)

- **T1** — No test for sprint lifecycle (create → start → add tasks → log burns → complete → verify burndown).
- **T2** — No test for room voting flow (create room → join → start voting → cast votes → reveal → accept).
- **T3** — No test for attachment upload/download/delete cycle.
- **T4** — No test for team scope filtering (create team → add root tasks → verify scope returns descendants).
- **T5** — No test for epic group snapshot (create epic → add tasks → snapshot → verify data).
- **T6** — No test for recurrence processing (set recurrence → advance date → verify task cloned).
- **T7** — No test for webhook dispatch (create webhook → trigger event → verify delivery attempted).
- **T8** — No test for audit log entries (perform actions → verify audit trail).
- **T9** — No frontend test for Timer component (start/pause/stop state transitions).
- **T10** — No frontend test for TaskList search/filter behavior.
- **T11** — No frontend test for auth flow (login → store token → restore on reload).
- **T12** — No test for CSV export format correctness (special characters, escaping).

## Documentation (D1–D5)

- **D1** — No API changelog — breaking changes between versions are undocumented.
- **D2** — No deployment guide — how to run behind nginx, configure CORS, set JWT secret.
- **D3** — No database schema documentation — ERD or table descriptions.
- **D4** — No WebSocket protocol documentation — room WS message format is undocumented.
- **D5** — No environment variable reference — `POMODORO_JWT_SECRET`, `POMODORO_SWAGGER`, `POMODORO_PORT` etc. are scattered across code.

## DevOps (O1–O3)

- **O1** — No database backup mechanism — SQLite file could be corrupted by unclean shutdown during write.
- **O2** — No health check for background tasks — if tick loop panics, server keeps running but timers stop.
- **O3** — No structured logging — uses `tracing` but no JSON formatter for log aggregation.

## Cleanup (C1–C5)

- **C1** — `routes/rooms.rs`: `POINT_CARDS` and `HOUR_CARDS` constants are defined in both `Rooms.tsx` and `EstimationRoomView.tsx`. Should be in `constants.ts` only.
- **C2** — `routes/tasks.rs`: Empty `// --- Comments ---` section marker with no code after it.
- **C3** — `store/types.ts`: `TimeReport` is a type alias for `BurnEntry` — confusing naming, should just use `BurnEntry` everywhere.
- **C4** — `routes/misc.rs`: `BurnTotalEntry` struct duplicates the frontend `BurnTotalEntry` interface. Should be in `db/types.rs`.
- **C5** — `Sprints.tsx`: `Column` component is defined inside `SprintView` function — recreated on every render. Should be extracted or memoized.

---

**Total: 102 items**
| Category | Count |
|---|---|
| Security | 10 |
| Bugs | 12 |
| Validation | 8 |
| Performance | 10 |
| Code Quality | 12 |
| Features | 15 |
| UX | 10 |
| Accessibility | 8 |
| Tests | 12 |
| Documentation | 5 |
| DevOps | 3 |
| Cleanup | 5 |

**Priority order:** Security → Bugs → Validation → Performance → Code Quality → Features → UX → Accessibility → Tests → Documentation → DevOps → Cleanup
