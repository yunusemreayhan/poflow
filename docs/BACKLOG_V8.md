# Backlog v8 — poflow

Generated: 2026-04-12
Backend: 186 tests | Frontend: 134 tests | TS strict: clean

---

## Security (S1–S6)

- **S1** — `auth.rs:secret()`: JWT secret generation reads `/dev/urandom` directly — panics if unavailable. SSE ticket generation (`misc.rs:create_sse_ticket`) has the same issue with `expect()`. Should use `getrandom` crate for portable CSPRNG.
- **S2** — `routes/attachments.rs`: Attachment storage key uses SHA-256 of `(timestamp, task_id, user_id, body_len)` — deterministic inputs. Two uploads in the same nanosecond with same params produce the same key, overwriting the file. Add a counter or random nonce.
- **S3** — `routes/export.rs:export_tasks`: Root users export all tasks, but non-root users are filtered by `user_id`. However, `export_sessions` always filters by `claims.user_id` even for root — inconsistent. Root should be able to export all sessions.
- **S4** — `routes/admin.rs:delete_user`: Reassigns deleted user's tasks/sessions to root, but doesn't revoke the deleted user's tokens. Deleted user's JWT remains valid until expiry.
- **S5** — `routes/epics.rs:add_sprint_root_tasks`: No authorization check — any authenticated user can add root tasks to any sprint. Should verify sprint ownership.
- **S6** — `store/api.ts:tryRefreshToken`: Accesses `useStore.getState().servers?.[0]` but the store has `savedServers` not `servers`. Refresh token flow is broken — never actually refreshes.

## Bugs (B1–B10)

- **B1** — `store/api.ts:tryRefreshToken`: References `server.refresh_token` from `useStore.getState().servers?.[0]` which doesn't exist (field is `savedServers`). Auto-refresh on 401 silently fails, forcing manual re-login.
- **B2** — `Timer.tsx:190`: Duplicate `aria-label` attribute on task select element — `aria-label="Select task for timer"` and `aria-label="Select task to focus on"` both present. Second one wins, first is dead code.
- **B3** — `AuditLog.tsx:34`: Duplicate `aria-label` on filter select — `aria-label="Filter audit log"` and `aria-label="Filter by entity type"` both present.
- **B4** — `engine.rs:tick()`: When `auto_start` creates a new session, it re-acquires the states lock inside the completions loop. If multiple users complete simultaneously, this creates sequential lock acquisitions per user instead of batching.
- **B5** — `Sprints.tsx:SprintSummary`: `done` variable shadows outer `done` array inside `byUser.map` destructuring `{ total, done }`. The retrospective section after the map uses the outer `done` correctly, but the shadowing is confusing and fragile.
- **B6** — `db/tasks.rs:get_descendant_ids`: Recursive CTE has no depth limit. A corrupted circular parent_id reference (despite validation) would cause infinite recursion in SQLite.
- **B7** — `CsvImport`: Import response field is `created` (from backend) but frontend reads `resp.imported` — always shows `undefined`. Should read `resp.created`.
- **B8** — `config.rs:save()`: Uses atomic write (tmp + rename) but doesn't fsync the tmp file before rename. On crash, the config file could be empty/corrupt.
- **B9** — `Rooms.tsx:RoomList`: `Room` type is used but never imported — relies on the type being available from the parent scope or global. Should have explicit import.
- **B10** — `EstimationRoomView.tsx:39`: `myVote` is used in the `useEffect` dependency comment but not in the actual deps array `[state?.room.current_task_id]`. If `myVote` changes without task change, the card selection won't update.

## Validation (V1–V5)

- **V1** — `routes/sprints.rs:add_sprint_tasks`: Validates tasks exist but doesn't check if tasks are already soft-deleted (`deleted_at IS NOT NULL`). Can add deleted tasks to sprints.
- **V2** — `routes/rooms.rs:start_voting`: Validates task exists but doesn't check if task is soft-deleted. Can start voting on deleted tasks.
- **V3** — `routes/burns.rs:log_burn`: Doesn't validate that `req.task_id` exists before checking sprint membership. A nonexistent task_id would pass the sprint_tasks check (not found = not in sprint) with a misleading error.
- **V4** — `routes/config.rs:update_config`: Doesn't validate `theme` field — accepts any string. Should validate against known themes ("dark", "light").
- **V5** — `routes/templates.rs:create_template`: No limit on template count per user or template data size. A user could create thousands of templates or store megabytes of JSON data.

## Performance (P1–P5)

- **P1** — `db/rooms.rs:get_room_state`: Fetches ALL tasks matching the room's project (up to 1000) on every room state request. For rooms without a project filter, fetches all leaf tasks. Should paginate or limit to sprint-scoped tasks.
- **P2** — `db/sessions.rs:get_history`: Loads all sessions then builds task paths with individual ancestor lookups. For 500 sessions, this is efficient (batch CTE), but the 500-session LIMIT is hardcoded with no pagination support.
- **P3** — `engine.rs:get_state`: Queries `get_today_completed_for_user` from DB on every idle state access. With frequent SSE polling, this creates unnecessary DB load. Should cache the daily count.
- **P4** — `db/sprints.rs:snapshot_sprint`: Fetches all sprint tasks, computes totals in Rust, then upserts. Could use a single SQL aggregate query instead of loading all task rows.
- **P5** — `store.ts:loadTasks`: The `tasksChanged` comparison iterates all tasks comparing `id` and `updated_at`. With 1000+ tasks and frequent SSE pushes, this runs often. Could compare a single hash/checksum instead.

## Code Quality (Q1–Q8)

- **Q1** — `TaskList.tsx`: 703 lines — `TaskNode` component is ~470 lines with 15+ state variables. Should decompose into `TaskRow`, `TaskActions`, `TaskDragHandler`.
- **Q2** — `App.tsx`: SSE connection logic is ~100 lines inline in a `useEffect`. Should extract to `useSseConnection` hook.
- **Q3** — `i18n.ts`: 673 lines with both locale definitions inline. Should split `en.ts` and `tr.ts` into separate files.
- **Q4** — `routes/mod.rs`: `is_owner_or_root` helper is used across many route files but defined in `types.rs`. Should be in a shared `auth_helpers` module with clear naming.
- **Q5** — `db/mod.rs:migrate()`: 300+ line function with 30+ CREATE TABLE statements and inline migrations. Should use a proper migration system (sqlx-migrate or numbered SQL files).
- **Q6** — `Sprints.tsx`: 497 lines with 3 components (`SprintList`, `SprintView`, `SprintSummary`) in one file. Should split into separate files.
- **Q7** — `routes/export.rs:import_tasks_csv`: Import response returns `{ created, errors }` but the frontend `CsvImport` component reads `resp.imported`. Field name mismatch.
- **Q8** — `store/store.ts`: `deleteTask` uses `showConfirm` which is a UI concern mixed into the store. Should be handled at the component level.

## Features (F1–F10)

- **F1** — No "trash" view for soft-deleted tasks. Users can restore via API but there's no UI to see or manage deleted tasks.
- **F2** — No sprint retro notes editor in UI. Backend supports `retro_notes` field on sprints but the frontend doesn't expose it for editing.
- **F3** — No task time tracking chart. Burn data exists per-task but there's no visualization of hours over time for individual tasks.
- **F4** — No keyboard shortcut for starting/stopping timer (global hotkey). Users must click buttons.
- **F5** — No "focus mode" that hides everything except the timer and current task. Useful for distraction-free work sessions.
- **F6** — No task cloning/duplication. Users must manually recreate similar tasks. Should have a "duplicate" action.
- **F7** — No sprint planning view with drag-and-drop from backlog to sprint. Current flow requires selecting tasks individually.
- **F8** — No dark/light theme CSS variables for charts. Recharts uses hardcoded colors that don't adapt to light theme.
- **F9** — No "my tasks" quick filter. Users must manually filter by assignee to see their own tasks.
- **F10** — No task status change notifications. When a team member completes a task, other assignees aren't notified.

## UX (U1–U6)

- **U1** — `AuthScreen.tsx`: Server URL editing is awkward — click to edit, blur to save. Should be a proper settings section or modal.
- **U2** — `Timer.tsx`: "Resume" button text is hardcoded English, not i18n'd. Same for "Short Break" and "Long Break" labels.
- **U3** — `Settings.tsx`: No visual feedback when config is saving (no loading spinner on save button).
- **U4** — `TaskList.tsx`: Sort dropdown uses raw `<select>` with no visual indicator of current sort. Should use the custom `Select` component for consistency.
- **U5** — `Sprints.tsx`: No empty state illustration for sprint detail when no tasks are added yet.
- **U6** — `CommentSection.tsx`: Optimistic comment uses `id: -Date.now()` which could collide if two comments are added within 1ms. Should use a UUID or incrementing counter.

## Accessibility (A1–A5)

- **A1** — `Timer.tsx:190`: Duplicate `aria-label` on select element. Remove the first one.
- **A2** — `EpicBurndown.tsx`: Area chart has no accessible data table fallback. Screen readers can't access the burndown data.
- **A3** — `SprintViews.tsx:BurndownView`: Burndown chart has no sr-only data table (unlike the weekly chart in History which has one).
- **A4** — `EstimationRoomView.tsx`: Voting cards use `role="radio"` but the container uses `role="radiogroup"` without `aria-label` on individual cards describing the full context.
- **A5** — `Rooms.tsx`: "Show closed" / "Hide closed" toggle button has no `aria-pressed` state for screen readers.

## Tests (T1–T8)

- **T1** — No test for soft-delete cascade behavior — verify that restoring a parent also restores children.
- **T2** — No test for concurrent room voting — two users voting simultaneously on the same task.
- **T3** — No test for CSV import with malformed data (missing columns, extra columns, special characters).
- **T4** — No test for webhook SSRF protection — verify private IP addresses are blocked.
- **T5** — No frontend test for Timer component state transitions (idle → running → paused → stopped).
- **T6** — No frontend test for TaskList search/filter/sort behavior.
- **T7** — No test for token refresh flow (access token expires → auto-refresh → retry request).
- **T8** — No test for rate limiting on auth endpoints.

## Documentation (D1–D3)

- **D1** — `docs/ENV_VARS.md`: Missing `POFLOW_ROOT_PASSWORD` documentation (it's referenced in code but not in the env vars doc).
- **D2** — `docs/API_CHANGELOG.md`: Doesn't document the `/api/import/tasks` response field name (`created` not `imported`).
- **D3** — No CONTRIBUTING.md with development setup instructions (how to run tests, code style, PR process).

## DevOps (O1–O2)

- **O1** — No database migration versioning beyond the single `schema_migrations` entry. Adding new columns requires careful `ALTER TABLE ... ADD COLUMN` with `.ok()` to ignore duplicates — fragile for production.
- **O2** — No rate limiting on non-auth endpoints. A malicious user could flood `/api/tasks` or `/api/sprints` with creation requests.

## Cleanup (C1–C4)

- **C1** — `store/api.ts:tryRefreshToken`: Dead code — references `useStore.getState().servers` which doesn't exist. The entire refresh flow is non-functional.
- **C2** — `Rooms.tsx`: `Room` type imported from parent scope but `import type { Room }` is missing. Works due to TypeScript's structural typing but should be explicit.
- **C3** — `routes/types.rs`: `UpdateSprintRequest` has a `status` field that's always rejected in `update_sprint`. Should remove the field or document why it exists.
- **C4** — `db/burns.rs`: `list_usernames` function is in the burns module but has nothing to do with burns. Should be in `users.rs`.

---

**Total: 63 items**
| Category | Count |
|---|---|
| Security | 6 |
| Bugs | 10 |
| Validation | 5 |
| Performance | 5 |
| Code Quality | 8 |
| Features | 10 |
| UX | 6 |
| Accessibility | 5 |
| Tests | 8 |
| Documentation | 3 |
| DevOps | 2 |
| Cleanup | 4 |

**Priority order:** Security → Bugs → Validation → Performance → Code Quality → Features → UX → Accessibility → Tests → Documentation → DevOps → Cleanup
