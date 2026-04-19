# Backlog v4 — poflow

Generated: 2026-04-11
Previous: v3 (78/78 complete), v2 (61/61 complete)
Test baseline: 97 backend, 44 frontend

---

## Bugs (22 items)

### Backend

**B1** — `tick()` uses global config instead of per-user config. All users share the same timer durations/auto-start settings. Should call `get_user_config(user_id)` per user inside the tick loop.

**B2** — `skip()` just calls `stop()`. Should advance to the next phase (e.g., skip break → start work), not reset to Idle.

**B3** — `md5_hash` uses `DefaultHasher` (SipHash), not a cryptographic hash. Token blocklist collision resistance is weak. Replace with actual SHA-256 truncated to 128 bits.

**B4** — Webhook HMAC signature uses `DefaultHasher`, not HMAC-SHA256. Receivers cannot trust signatures. Replace with `hmac-sha256` crate.

**B5** — `cancel_burn` ignores sprint_id from URL path. A burn from sprint A can be cancelled via `/api/sprints/999/burns/{burn_id}`. Should validate sprint_id matches the burn's sprint.

**B6** — `leave_room` doesn't notify SSE. Other users won't see the member leave in real-time. Same for `start_voting`.

**B7** — `add_assignee` / `remove_assignee` / `add_comment` / `delete_comment` don't notify SSE. UI won't update in real-time for other users.

**B8** — `team_id` filter dropped in task count query. `x-total-count` header is wrong when filtering by team — returns unfiltered count.

**B9** — Rate limiter bypassed when no IP header present. `check_auth_rate_limit` returns `Ok(())` when no `x-forwarded-for`/`x-real-ip` header exists, completely skipping rate limiting for direct connections.

**B10** — `retro_notes` textarea uses `defaultValue`. Won't update after SSE push — becomes stale after first render. Should use controlled `value` with `onChange`.

**B11** — `stop_session` silently ignores DB errors via `.ok()`. Failed session end leaves "running" sessions in DB forever. Should log the error.

**B12** — `daily_completed` counter drifts after server restart. In-memory count resets but DB count doesn't, causing inconsistency.

### Frontend

**B13** — Attachment upload/download uses `(window as any).__serverUrl` and `__token` which are never set. Auth will fail. Should use `useStore.getState()`.

**B14** — Attachment download `<a href>` has no auth header. Server rejects as unauthenticated. Need to use fetch + blob URL or ticket mechanism.

**B15** — `BurnsView` initial `taskId` can be 0 when tasks array is empty. Submitting sends invalid task_id to API.

**B16** — `doReveal` countdown in EstimationRoomView fires on unmounted component. No cleanup on navigation away.

**B17** — `customAccept` parsed with `parseFloat` without NaN check. NaN gets sent to API.

**B18** — Sprint delete has no confirmation dialog. One misclick permanently deletes sprint and all data.

**B19** — Move up/down in context menu uses two separate `updateTask` calls creating a race condition. SSE reload between calls shows inconsistent state.

**B20** — SSE debounce timer in Sprints.tsx leaks on unmount. `clearTimeout` only clears previous debounce, not the pending one.

**B21** — `AdminPanel.load` uses `.then(setUsers)` without null check. If `apiCall` returns undefined, `setUsers(undefined)` causes `.map()` crash.

**B22** — `TeamManager` fetches `/api/admin/users` which returns 403 for non-root users. Should use `/api/users` or handle error.

---

## Security (14 items)

**S1** — JWT secret file created with default permissions (0644). Any system user can read it and forge tokens. Set to 0600.

**S2** — No `token_type` field in JWT Claims. Refresh tokens can be used as access tokens directly. Add `typ: "access"|"refresh"` claim and validate.

**S3** — Refresh token endpoint has no rate limiting. Stolen refresh token allows rapid token generation.

**S4** — SSE still accepts raw JWT in `?token=` query parameter. Tokens appear in server logs, browser history, proxy logs. Remove legacy path, require ticket only.

**S5** — IP spoofing via `X-Forwarded-For`. Rate limiter trusts client-supplied headers. Should use peer address or only trust headers behind known proxy.

**S6** — Webhook URLs not validated for SSRF. Can target internal endpoints (`127.0.0.1`, `169.254.169.254`). Add private IP block check (partially done in test but not in `create_webhook` handler).

**S7** — JWT secret fallback uses `DefaultHasher` with low entropy when `/dev/urandom` unavailable. Not a CSPRNG. Should fail hard instead of using weak fallback.

**S8** — `update_profile` doesn't validate username format. Registration validates (alphanumeric + underscore + hyphen, max 32), but profile update doesn't.

**S9** — No password length upper bound. Extremely long passwords cause excessive bcrypt CPU. Cap at 128 chars.

**S10** — XOR "encryption" for Tauri auth storage is trivially reversible. Key derived from hostname+username is deterministic. Consider OS keychain integration.

**S11** — Config file and DB file written with default permissions. Should restrict to 0600.

**S12** — `write_file` in Tauri uses synchronous `std::fs::write` blocking the async runtime. Use `tokio::fs::write`.

**S13** — `api_call` in Tauri returns raw server error text, potentially leaking internal details.

**S14** — No HTTPS enforcement in Tauri. If user sets remote server URL, JWT tokens sent in plaintext.

---

## Features (16 items)

**F1** — Task timer selector. Timer view has no way to pick which task to focus on. Add a task dropdown/picker in the Timer component.

**F2** — Leave room button. Regular members have no way to leave an estimation room from the UI. Only admin can close.

**F3** — Break duration display. "Short Break" and "Long Break" buttons don't show how many minutes each is.

**F4** — Password visibility toggle on auth screen. Common UX expectation for login/register forms.

**F5** — Sprint confirmation dialogs. Add confirmation for sprint delete, start, and complete actions.

**F6** — Audit logging for sprint/room/burn operations. Currently only task CRUD has audit trail.

**F7** — Webhook dispatch for sprint/room/burn events. Currently only task events trigger webhooks.

**F8** — Task reorder API with single batch call. Current move up/down uses two separate update calls. Add `POST /api/tasks/reorder` with `[{id, sort_order}]` body.

**F9** — Retry logic for failed webhook deliveries. Currently fire-and-forget with no retry or dead-letter queue.

**F10** — Multiple i18n locales. Architecture exists but only English is implemented. Add at least one more locale (e.g., Turkish, German).

**F11** — String interpolation and pluralization in i18n. Current system has no way to insert variables or handle plural forms.

**F12** — Template form builder. Current template UI requires raw JSON editing. Add a structured form.

**F13** — Sprint scope indicator in burn form. No sprint name context shown when logging burns.

**F14** — Session note on timer. "Hide/Add session note" exists but needs better UX — show note input inline.

**F15** — Shared reqwest client for webhook dispatch. Currently creates a new HTTP client per dispatch — no connection pooling.

**F16** — Atomic config file writes. `std::fs::write` can corrupt config on crash. Write to temp file and rename.

---

## Performance (14 items)

**P1** — `tick()` locks entire states HashMap every second for all users. Use per-user tokio timers or timer wheel for scalability.

**P2** — `TaskNode` subscribes to 10+ individual store selectors. Any store change triggers re-render checks for every visible node. Use shallow equality or combined selector.

**P3** — `taskSprints.filter(ts => ts.task_id === t.id)` runs O(n×m) on every render. Pre-compute `Map<taskId, sprints[]>` in the store.

**P4** — `buildTree` rebuilds on every `tasks` array reference change. Since `loadTasks` creates a new array each time, tree rebuilds even when data is identical. Use structural comparison or stable references.

**P5** — `DetailNode` makes 4 API calls per task on mount. For a task with 20 children, that's 80+ calls. Batch into a single endpoint.

**P6** — 60 SVG tick marks in Timer re-render every second. Memoize static tick marks separately from the animated ring.

**P7** — 365 `HeatmapCell` components each with Framer Motion animation. Expensive for the history view. Use CSS transitions or virtualize.

**P8** — `get_tasks_full` runs 5 sequential DB queries for ETag. Combine into a single query.

**P9** — `get_tasks_full` loads ALL tasks into memory without pagination. Problematic for large datasets.

**P10** — SSE ticket cleanup is O(n) on every ticket creation. Under high concurrency, lock contention is an issue.

**P11** — Rate limiter HashMap grows unbounded. No periodic cleanup of stale IP entries. Memory leak over time.

**P12** — `AnimatePresence` wraps every child in the task tree. Framer Motion tracks all children for exit animations — expensive with hundreds of tasks.

**P13** — `EstimationRoomView` full state reload on every SSE event. Fetches entire room state even if only one vote changed.

**P14** — Multiple `useEffect(load, [])` in Settings sub-components. Each fires its own API call on mount — no batching.

---

## Code Quality (14 items)

**Q1** — `PRIORITY_COLORS` duplicated in `TaskList.tsx` and `TaskContextMenu.tsx`. Extract to shared constant.

**Q2** — `(window as any)` used for global state (`__ctxCacheTime`, `__tasksLoadedAt`). Move to Zustand store or module-level variables.

**Q3** — Magic numbers throughout frontend (5000, 10000, 30000, 260, 400, 520). Extract to named constants.

**Q4** — Inconsistent error handling patterns. Mix of `try/catch`, `.catch(() => {})`, `.catch(() => [])`, and no handling. Standardize.

**Q5** — Store interface is a single monolithic type with 40+ members. Split into slices (auth, tasks, timer, UI).

**Q6** — `CommentSection` exported from `TaskDetailView.tsx` and imported in `TaskList.tsx`. Extract to own file.

**Q7** — Inline SSE debounce pattern duplicated in `Sprints` and `SprintView`. Extract to `useSseDebounce` custom hook.

**Q8** — Magic status strings scattered as literals ("backlog", "active", "completed", "planning", "in_progress"). Define as constants/enums.

**Q9** — `useEffect` cleanup missing for async operations in `DetailNode`, `CommentSection`, `TaskAttachments`. Can setState on unmounted component.

**Q10** — Unused imports: recharts in `Sprints.tsx` (moved to SprintViews), `InlineAddSubtask` in `TaskList.tsx`.

**Q11** — `prevPhase` ref in Timer.tsx is set but never read. Dead code.

**Q12** — `accept_estimate` in rooms.rs uses `.filter().filter().next()` instead of `.find()`.

**Q13** — Snapshot errors silently ignored with `let _ = db::snapshot_sprint(...)`. Should log errors.

**Q14** — Inconsistent HTTP status codes. `join_room` returns 200, `kick_member` returns 204. Standardize.

---

## UX (14 items)

**U1** — No loading state for sprint list. Empty state ("No sprints yet") flashes before data loads. Add skeleton/spinner.

**U2** — Keyboard shortcut `r` triggers refresh when focused on non-input elements (buttons, etc.). Guard should check `document.activeElement` more broadly.

**U3** — Double `?` shortcut handler. Global shortcuts effect shows toast AND shortcuts panel toggles. Remove duplicate.

**U4** — No keyboard shortcut for timer tab. Keys 1-6 map to 6 tabs but timer (index 0) has no shortcut.

**U5** — Double-click to rename gives no feedback for non-owners. Should show "not allowed" cursor or tooltip.

**U6** — Expand-all button remounts entire tree, destroying all local state (expanded, inline editors, context menus).

**U7** — Search input placeholder says "regex" which is confusing for non-technical users. Change to "Search tasks...".

**U8** — Context menu submenu doesn't check vertical overflow. Long user lists overflow below viewport.

**U9** — No loading indicator for context menu data. Right-click fetches sprints/users but menu appears immediately with empty data.

**U10** — Board drag-and-drop has no visual feedback on the dragged item. Only drop target highlights.

**U11** — No confirmation before logging time. Accidental Enter in time report input logs hours immediately.

**U12** — Heatmap not responsive. 365 cells with flex-wrap breaks on narrow screens.

**U13** — Server URL editing in AuthScreen is hidden. Must click URL text to reveal input — not discoverable.

**U14** — Countdown overlay in EstimationRoomView blocks entire screen for 3 seconds with no cancel option.

---

## Accessibility (16 items)

**A1** — Context menu has no keyboard navigation. No keyboard trigger (Shift+F10), no arrow keys, no focus trapping, no Escape to close.

**A2** — Context menu lacks ARIA roles. Needs `role="menu"` on container, `role="menuitem"` on buttons.

**A3** — Drag-and-drop is mouse-only everywhere (task tree, sprint board). No keyboard alternative for reordering.

**A4** — Sprint list items are clickable divs with no `role="button"`, `tabIndex`, or keyboard handler.

**A5** — Toggle component in Settings has no `role="switch"` or `aria-checked`. Screen readers can't determine state.

**A6** — `NumInput` in Settings has no associated `<label>`. `Field` renders a `<span>` but doesn't use `htmlFor`/`id`.

**A7** — Timer control buttons (Pause, Stop, Skip) have icons only, no `aria-label`.

**A8** — Daily goal dots in Timer have no text alternative for screen readers.

**A9** — Voting cards in EstimationRoomView lack `aria-label` (e.g., "Vote 5 points").

**A10** — Heatmap uses `role="gridcell"` but has no parent `role="grid"` or `role="row"`. Invalid ARIA structure.

**A11** — Auth form inputs use only `placeholder` — no `<label>` elements. Placeholder disappears on input.

**A12** — Error messages in AuthScreen lack `role="alert"` or `aria-live`. Screen readers won't announce errors.

**A13** — Shortcuts panel overlay doesn't trap focus. Users can tab into background content.

**A14** — Bulk checkboxes use `opacity: 0` to hide. Still focusable — keyboard users land on invisible elements.

**A15** — `EditField` click-to-edit in TaskDetailView is not keyboard accessible. No `tabIndex`, no `role="button"`.

**A16** — Countdown overlay in EstimationRoomView has no `aria-live` region. Screen readers won't announce countdown.

---

## Validation (12 items)

**V1** — No bounds checking on config values. `work_duration_min: 0` creates instant completion loop. `daily_goal: 0` means goal always met. Add min/max validation.

**V2** — `estimation_mode` accepts any string. Validate against `["hours", "points"]`.

**V3** — `create_sprint` has no name validation. Empty name and no length limit.

**V4** — `create_room` doesn't validate `room_type` or `estimation_unit`. Arbitrary strings accepted.

**V5** — `cast_vote` doesn't validate vote value. Negative or extremely large values accepted.

**V6** — `log_burn` doesn't validate points/hours are non-negative. Negative values can be logged.

**V7** — `add_time_report` doesn't validate hours are positive. Zero or negative hours accepted.

**V8** — Date fields (`due_date`, `start_date`, `end_date`) accept arbitrary strings with no format validation.

**V9** — `AddSprintTasksRequest.task_ids` has no size limit. Thousands of IDs in one request.

**V10** — `update_sprint` allows direct status manipulation, bypassing start/complete lifecycle.

**V11** — No validation that tasks exist before adding to sprint. Invalid task_ids silently accepted.

**V12** — `add_comment` has no content validation. Empty/blank comments can be posted.

---

## Authorization (6 items)

**Z1** — `remove_sprint_task` has no authorization. Any authenticated user can remove tasks from any sprint.

**Z2** — `add_sprint_tasks` has no ownership check. Any user can add tasks to any sprint.

**Z3** — `reorder_tasks` has no ownership check. Any user can reorder any tasks.

**Z4** — `add_time_report` has no task ownership/assignee check. Any user can log time against any task.

**Z5** — `log_burn` has no sprint ownership check. Any user can log burns to any sprint.

**Z6** — `add_assignee` has no authorization. Any user can assign anyone to any task.

---

## Tests (10 items)

**T1** — Test `tick()` per-user config isolation. Verify two users with different `work_duration_min` get correct durations.

**T2** — Test `skip()` advances to next phase instead of stopping.

**T3** — Test webhook HMAC signature verification (after S4 fix).

**T4** — Test `cancel_burn` validates sprint_id matches (after B5 fix).

**T5** — Test rate limiter with direct connections (no X-Forwarded-For header).

**T6** — Test refresh token cannot be used as access token (after S2 fix).

**T7** — Test config validation bounds (after V1 fix).

**T8** — Test authorization on sprint task add/remove (after Z1/Z2 fix).

**T9** — Frontend: test SSE reconnection behavior.

**T10** — Frontend: test attachment upload/download flow (after B13/B14 fix).

---

## Documentation (4 items)

**D1** — Document lock ordering in engine.rs. `tick()` acquires config→states, `get_state()` acquires states→config. Potential deadlock pattern.

**D2** — Document SSE event types and when they fire. Currently inconsistent — some mutations notify, some don't.

**D3** — Document authorization model. Which endpoints require ownership vs any authenticated user.

**D4** — Add CHANGELOG.md tracking v1→v2→v3→v4 changes.

---

## i18n (4 items)

**I1** — Extract all hardcoded strings in Sprints.tsx, SprintViews.tsx, Timer.tsx, EstimationRoomView.tsx, History.tsx, Settings.tsx to i18n keys. (~200 strings)

**I2** — Add interpolation support to i18n system. Need `{count}` style variable substitution.

**I3** — Add pluralization support. "1 session" vs "2 sessions".

**I4** — Add at least one additional locale (Turkish or German).

---

## Summary

| Category | Count |
|---|---|
| Bugs | 22 |
| Security | 14 |
| Features | 16 |
| Performance | 14 |
| Code Quality | 14 |
| UX | 14 |
| Accessibility | 16 |
| Validation | 12 |
| Authorization | 6 |
| Tests | 10 |
| Documentation | 4 |
| i18n | 4 |
| **Total** | **146** |

## Priority Order

1. **Security** S1-S14 (especially S1, S2, S3, S4, S6, S7)
2. **Bugs** B1-B22 (especially B1, B2, B3, B4, B13, B14)
3. **Authorization** Z1-Z6
4. **Validation** V1-V12
5. **Features** F1-F16
6. **Performance** P1-P14
7. **Code Quality** Q1-Q14
8. **UX** U1-U14
9. **Accessibility** A1-A16
10. **Tests** T1-T10 (write alongside fixes)
11. **Documentation** D1-D4
12. **i18n** I1-I4
