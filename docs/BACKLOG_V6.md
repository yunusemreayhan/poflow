# Backlog v6

Fresh codebase audit. 150 items across 13 categories.
Priority order: Security → Bugs → Validation → Performance → Code Quality → Features → UX → Accessibility → i18n → Tests → Documentation → DevOps → Cleanup.

---

## Security (15)

**S1** — Rate limiter defined in `lib.rs` but never `.layer()`'d onto any route. Auth endpoints have zero rate limiting. Brute-force login is trivial.

**S2** — `X-Forwarded-For` header used for rate limiting is trivially spoofable. Should use actual peer address from `ConnectInfo` as primary, with `X-Forwarded-For` only behind a trusted reverse proxy flag.

**S3** — Webhook dispatch has no SSRF protection beyond URL scheme check. Server makes HTTP requests to user-provided URLs. Should resolve DNS and reject private/loopback IPs before connecting.

**S4** — CSV injection in `escape_csv`: values starting with `=`, `+`, `-`, `@` can execute formulas in Excel/LibreOffice. Prefix with `'` or tab character.

**S5** — `export_tasks` for non-root users fetches ALL tasks from DB then filters in Rust. Leaks timing information and wastes resources. Filter at DB level.

**S6** — JWT has no `iat` (issued-at) claim. Cannot implement "revoke all tokens before time X" for password changes or security incidents.

**S7** — Refresh token endpoint propagates stale `username`/`role` from old token claims. If admin changes a user's role, the refreshed token still carries the old role.

**S8** — `auth.rs` `md5_hash` function uses `{:x}` format on `u128` which doesn't zero-pad. Leading-zero hashes produce shorter strings. Use `{:032x}`.

**S9** — Token blocklist is an unbounded in-memory `HashSet` behind a `tokio::Mutex`. Every revoked token persists forever until restart. Reads (every authenticated request) contend on the mutex. Use `DashMap` or `RwLock`.

**S10** — `seed_root_user` creates default `root/root` credentials with no forced password change on first login.

**S11** — `write_file` in Tauri bridge: blocked extension list is incomplete (missing `.py`, `.pl`, `.rb`, `.jar`, `.deb`, `.rpm`, `.AppImage`). Path traversal check doesn't canonicalize (symlinks bypass `..` check).

**S12** — `api_call` in Tauri bridge sends JWT to whatever `base_url` is configured. If user changes server URL to a malicious host, token is leaked. No HTTPS enforcement warning in UI.

**S13** — No `Content-Security-Policy`, `X-Frame-Options`, or `Strict-Transport-Security` headers on API responses (only on attachment downloads).

**S14** — Swagger UI unconditionally exposed in production. Should be behind an env flag.

**S15** — `config.rs` and `auth.rs` tilde fallback: `PathBuf::from("~/.config")` creates a literal `~` directory. Tilde expansion is a shell feature, not filesystem.

---

## Bugs (20)

**B1** — `TaskList.tsx` line ~218: duplicate `className` attribute on title div. Second `className` overwrites the first, losing all title styling (font-semibold, line-through, truncate, text color).

**B2** — `TaskContextMenu.tsx` status "active" maps to "WIP" label but backend uses `"in_progress"`. Tasks set to "active" from context menu won't appear in sprint board "In Progress" column.

**B3** — `engine.rs` division by zero if `long_break_interval` is 0. `session_count % config.long_break_interval` panics. No validation on this config field.

**B4** — `App.tsx` keyboard shortcut `tabMap` maps `"5"` to `"settings"` and `"6"` to `"api"` — swapped from visual tab order (api is tab 5, settings is tab 6).

**B5** — `Sprints.tsx` duplicate empty-state: two `!loading && sprints.length === 0` blocks render simultaneously (text version + emoji version).

**B6** — `import_tasks_csv` naive CSV parser splits on commas, breaking on quoted fields containing commas. Can't round-trip with `escape_csv` output.

**B7** — `watch::Sender` in engine broadcasts only one user's state. In multi-user scenarios, SSE subscribers may miss their own updates (overwritten by another user's state before read).

**B8** — `completed_states[i]` potential index-out-of-bounds in tick() Phase 2 if a user's state was removed between building `comps` and collecting `completed_states`.

**B9** — `main.rs` due-date reminder `notified` set is in-memory only. On restart, all due tasks get re-notified. The `len() > 1000` cleanup causes re-notifications.

**B10** — `main.rs` monthly recurrence `d.day().min(28)` permanently shifts tasks on 29th/30th/31st to 28th after first recurrence.

**B11** — `reorder_tasks` ownership check only validates the first task in the array. A user can reorder other users' tasks by putting one of their own first.

**B12** — `count_tasks` doesn't handle `assignee` or `team_id` filters, so paginated results with those filters show incorrect total counts.

**B13** — `delete_task` does 7+ queries without a transaction. Server crash mid-deletion leaves orphaned records.

**B14** — `update_task` read-modify-write without transaction. Concurrent updates can be lost (optimistic locking only covers explicit `expected_updated_at` usage).

**B15** — `sprint create` date validation only checks `d.len() != 10`, accepting any 10-char string like `"aaaaaaaaaa"`. Should use `NaiveDate::parse_from_str`.

**B16** — `EstimationRoomView` `doReveal` checks `state?.room.status` after countdown but `state` is a stale closure capture. Race condition with concurrent reveals.

**B17** — `cast_vote` doesn't verify user is a room member. Any authenticated user can vote in any room.

**B18** — `notify.rs` `send_notification` makes blocking D-Bus IPC call on async runtime. Should use `spawn_blocking`.

**B19** — `History.tsx` CSV export creates blob URL that's never revoked (memory leak). Also doesn't escape quotes/newlines in task titles.

**B20** — `store.ts` `deleteTask` calls `showConfirm` which is async-deferred, but the function returns immediately. Callers that `await deleteTask(id)` get resolved promise before user sees dialog.

---

## Validation (12)

**V1** — No max length on task `title`, `description`, `project`, `tags`. Unbounded strings go to DB. Add server-side limits (title: 500, description: 10000, project: 200, tags: 500).

**V2** — No max length on sprint `goal`, `retro_notes`. Add limits (goal: 1000, retro_notes: 10000).

**V3** — No max length on comment `content`. Add limit (10000 chars).

**V4** — `import_tasks_csv` has no size limit on CSV body. Add max 1MB.

**V5** — `import_tasks_csv` doesn't validate priority range (1-5) from CSV data.

**V6** — `bulk_update_status` uses different valid statuses than `validate_task_status`. Inconsistent — should reuse the shared validator.

**V7** — `update_task` allows setting `parent_id` to create circular references (A→B→A). No cycle detection.

**V8** — `start_voting` doesn't verify the task exists or belongs to the room's project.

**V9** — `config.rs` no validation: `long_break_interval` can be 0 (causes panic), `work_duration_min` can be 0 (0-second timer), no upper bounds.

**V10** — `NumInput` in Settings allows typing values outside min/max via keyboard. HTML min/max only constrain the stepper.

**V11** — `AddTimeReportRequest` has no max hours validation. User can log 999999 hours.

**V12** — `LogBurnRequest` has no validation on `points` or `hours` range beyond non-negative.

---

## Performance (12)

**P1** — `bulk_update_status` does N+1 queries (SELECT + UPDATE per task). Should be single UPDATE with IN clause after bulk ownership check.

**P2** — `reorder_tasks` does N individual UPDATEs (up to 500). Should use transaction with single prepared statement.

**P3** — `add_sprint_tasks` does N+1 queries for validation (SELECT per task_id). Should batch validate.

**P4** — `TaskNode` subscribes to 10+ individual Zustand selectors. With 500 tasks = 6500+ subscriptions. Use `useShallow` or single selector.

**P5** — `motion.div` with `layout` prop on every TaskNode causes layout thrashing on tree changes. Remove `layout` or use `layoutId` selectively.

**P6** — `DetailNode` fires 4 parallel API calls per node on mount. For 20-node tree = 80 API calls. Batch or cache.

**P7** — `computeRollup` called recursively per `DetailNode` — O(n²) for the tree. Should compute once at top and pass down.

**P8** — `is_revoked()` acquires `tokio::Mutex` on every authenticated request. Use `DashMap` or `RwLock` for read-heavy workload.

**P9** — `recover_interrupted` fetches all running sessions then updates one by one. Should be single UPDATE statement.

**P10** — No index on `tasks(user_id)`, `comments(task_id)`, `rooms(creator_id)`. Add indexes for ownership checks.

**P11** — Tab content fully unmounted/remounted on every tab switch (AnimatePresence). TaskList loses scroll position, expanded state, search query. Keep mounted but hidden.

**P12** — `SprintView.load()` makes 3 sequential API calls on every SSE event. Should use `Promise.all` and add ETag caching.

---

## Code Quality (12)

**Q1** — `auth.rs` function named `md5_hash` actually uses SHA-256 truncated to 128 bits. Rename to `token_hash` or `sha256_short`.

**Q2** — `cors_origins` config field is defined but never read by `build_router`. Dead code — either use it or remove it.

**Q3** — `main.rs` double recovery: calls `db::recover_interrupted` then `Engine::new` calls `db::recover_interrupted_sessions`. Redundant.

**Q4** — All `Engine` fields are `pub`. External code can bypass invariants. Make fields private, expose via methods.

**Q5** — `TimeReport` type alias in `api.ts` is legacy. Remove and migrate all usages to `BurnEntry`.

**Q6** — `api.ts` mixes HTTP logic with 300+ lines of type definitions. Split into `api.ts` and `types.ts`.

**Q7** — `TaskNode` is 350+ lines. Decompose into TaskRow, TaskActions, TaskMeta sub-components.

**Q8** — `ctxSprints` and `ctxUsers` are local state per TaskNode but represent global data. Move to store.

**Q9** — `theme` state is triple-sourced: localStorage, component state, server config. Consolidate to single source of truth.

**Q10** — `delete_task` manually deletes from 5 tables then relies on CASCADE for the rest. Redundant — all tables have CASCADE. Remove manual cleanup, wrap in transaction.

**Q11** — No token refresh logic. Store saves `refresh_token` but never uses it. After 7-day JWT expiry, user is silently logged out.

**Q12** — `snapshot_sprint` uses `estimated` (poflow count) as "total_points". Semantically confusing — burndown shows poflow counts labeled as "points".

---

## Features (14)

**F1** — Health check endpoint (`GET /api/health`) for monitoring and load balancer checks.

**F2** — Token refresh: auto-refresh JWT before expiry using stored refresh token. Show re-login prompt only on refresh failure.

**F3** — "Select all visible" button for bulk task operations.

**F4** — Task search should use plain substring match by default, regex only with `/pattern/` syntax. Current regex causes errors on `[`, `(` etc.

**F5** — Sprint board: keyboard-accessible card movement (arrow keys to move between columns).

**F6** — Estimation room: show which card the user previously voted (persist `selectedCard` across remounts).

**F7** — History: date range filter and pagination for recent sessions (currently hardcoded to 20).

**F8** — Confirm dialog: dynamic confirm button text (not always "Delete"). Pass `confirmLabel` to `showConfirm`.

**F9** — Timer: audio/visual feedback on poflow completion in frontend (backend sends desktop notification but frontend has no in-app feedback).

**F10** — Timer: persist selected task across tab switches (currently resets to undefined on remount).

**F11** — Estimation room: cancel reveal countdown (currently the async function continues even after overlay is dismissed).

**F12** — Sprint board: visual drop indicator showing where card will land (not just column highlight).

**F13** — Task detail: navigation stack (back goes to parent detail, not all the way to task list).

**F14** — Task detail: status field should be dropdown with valid statuses, not free-text EditField.

---

## UX (12)

**U1** — No loading states on mutations across all components. Create/update/delete operations lack spinners, enabling double-submissions.

**U2** — Context menu and sprint board drag-and-drop are mouse-only. No touch device support for submenus (hover-only).

**U3** — Shortcuts panel doesn't list tab-switching shortcuts (0-6) or `r` refresh shortcut.

**U4** — Toast click-to-dismiss has no `×` icon. Users may not realize they can click to dismiss.

**U5** — Retro notes textarea uses `defaultValue` with `key` hack — loses cursor position on SSE-triggered reload while typing.

**U6** — Settings: no unsaved changes indicator. Users can navigate away without saving.

**U7** — Sprint date inputs have no visible labels, just placeholders that disappear on focus.

**U8** — Heatmap cells are 3x3px — extremely small touch targets. Not aligned to weekday rows like GitHub's contribution graph.

**U9** — Empty state message says "No projects yet" even when search filter returns no results. Should say "No matching tasks".

**U10** — Team delete button is inline `×` inside team name chip — easy to accidentally click when selecting.

**U11** — `EditField` in TaskDetailView has no `onBlur` save — clicking away loses edits silently.

**U12** — Estimation room: no loading state on vote/reveal/accept actions. Users can double-vote or double-reveal.

---

## Accessibility (12)

**A1** — Sprint tabs have no `role="tab"`, `aria-selected`, or `role="tablist"`. Same for estimation room tabs.

**A2** — Drag-and-drop (task reorder, sprint board) has no keyboard alternative.

**A3** — Context menu has no arrow-key navigation, no `aria-activedescendant`, no roving tabindex.

**A4** — Charts (burndown, velocity, heatmap, weekly bar) have no accessible data table alternative.

**A5** — Sidebar team selector uses truncated text (`t.name.slice(0, 4)`) — screen readers read truncated text. Needs `aria-label={t.name}`.

**A6** — History summary cards use emoji as semantic indicators (🍅, ⏱️, 🔥). Screen readers say "tomato", "stopwatch", "fire".

**A7** — Heatmap has 365 `tabIndex={0}` cells. Tab-navigating through all is unusable. Use `role="grid"` with row/column navigation.

**A8** — Timer phase changes not announced to screen readers. Add `aria-live` region for phase label.

**A9** — Color-only phase indication in Timer (red=work, teal=break, blue=long break). Add text/icon differentiation for color-blind users.

**A10** — `Toggle` component in Settings has no `htmlFor`/`id` linking between label and control.

**A11** — Estimation room vote cards lack `role="radio"` or `role="option"` group semantics.

**A12** — `EditField` in TaskDetailView uses `role="button"` on a `<div>`. Should be a `<button>` element.

---

## i18n (8)

**I1** — i18n system is ~95% unused. The `Locale` interface has 90+ keys but <5% of UI strings use them. Massive hardcoded English strings across all components.

**I2** — `Sprints.tsx`: ~40 hardcoded strings (Sprint name, Board, Backlog, Burns, Burndown, Summary, Start, Complete, Retro Notes, etc.).

**I3** — `EstimationRoomView.tsx`: ~30 hardcoded strings (Pick your estimate, Reveal Cards, Consensus, No consensus, Average, etc.).

**I4** — `TaskContextMenu.tsx`: ~25 hardcoded strings (Status, Todo, WIP, Done, Archive, Priority, Move up/down, Start timer, etc.).

**I5** — `History.tsx`: ~15 hardcoded strings (Total Sessions, Focus Hours, Current Streak, Activity, This Week, etc.).

**I6** — `SprintViews.tsx`: ~15 hardcoded strings (Log Burn, Points, Hours, No burns logged, Velocity Trend, etc.).

**I7** — `Settings.tsx`: ~20 hardcoded strings (Timer Durations, Automation, Notifications, Goals, Account, Server, etc.).

**I8** — `AuthScreen.tsx`: ~10 hardcoded strings (Create your account, Sign in to continue, First user becomes admin, etc.).

---

## Tests (10)

**T1** — No test for rate limiter (it's not even wired up — see S1).

**T2** — No test for circular parent_id detection (see V7).

**T3** — No test for CSV import with quoted fields containing commas (see B6).

**T4** — No test for `long_break_interval = 0` panic (see B3).

**T5** — No test for bulk_update_status ownership isolation (user A can't bulk-update user B's tasks).

**T6** — No test for concurrent timer operations (start/stop/pause race conditions).

**T7** — No test for WebSocket keepalive ping/pong.

**T8** — No test for token refresh flow (refresh → new access token → old token revoked).

**T9** — No frontend test for ErrorBoundary component.

**T10** — No frontend test for SSE reconnect with exponential backoff.

---

## Documentation (5)

**D1** — No API changelog documenting breaking changes between versions.

**D2** — No architecture diagram showing component relationships (Engine ↔ DB ↔ Routes ↔ SSE ↔ WebSocket).

**D3** — No developer setup guide (prerequisites, build steps, test commands, IDE config).

**D4** — Lock ordering documentation in `engine.rs` is incorrect — `pause()`/`resume()` violate the documented order.

**D5** — No runbook for common operational tasks (backup, restore, user management, log analysis).

---

## DevOps (3)

**O1** — No graceful shutdown for background tasks (tick loop, snapshot loop, recurrence loop). They're abruptly terminated on SIGTERM, potentially mid-DB-write.

**O2** — Hourly sprint snapshot fires immediately on startup (tokio interval ticks on first call). Creates extra snapshot entry.

**O3** — No database migration versioning. Uses `CREATE TABLE IF NOT EXISTS` + `ALTER TABLE ADD COLUMN` with `.ok()`. Silent failures if ALTER fails for non-duplicate reasons.

---

## Cleanup (5)

**C1** — `Sprints.tsx` imports `AreaChart`, `Area`, `XAxis`, `YAxis`, `Tooltip`, `ResponsiveContainer` but charts are in `SprintViews.tsx`. Dead imports.

**C2** — `BurnTotal` interface in `api.ts` is defined but never used (replaced by `BurnTotalEntry`).

**C3** — `activeTasks` filter in `Timer.tsx` runs on every render without `useMemo`.

**C4** — `setTheme` parameter `t` shadows the translation function `t` from `useT()` in `App.tsx`.

**C5** — `config.rs` `save()` sets file permissions after rename (brief world-readable window). Set permissions on temp file before rename.

---

**Total: 150 items**
- Security: 15
- Bugs: 20
- Validation: 12
- Performance: 12
- Code Quality: 12
- Features: 14
- UX: 12
- Accessibility: 12
- i18n: 8
- Tests: 10
- Documentation: 5
- DevOps: 3
- Cleanup: 5
