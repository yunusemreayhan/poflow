# BACKLOG v23 — Fresh Codebase Audit (2026-04-12)

Full audit of 58 backend .rs files (~6800 LOC), 66 frontend .ts/.tsx files (~9300 LOC), 275 backend tests, 154 frontend tests.

## Security (2 items)

- [ ] **S1.** `getFreshToken()` in `api.ts` calls `/api/health` to verify the token is valid before returning it. But `/api/health` doesn't require auth — it always returns 200. The health check doesn't actually verify the token. Should call an authenticated endpoint like `GET /api/timer` instead.

- [ ] **S2.** `savedServers` in `store.ts` stores tokens (including refresh tokens) in `localStorage` via `saveServers()`. These persist across sessions and are accessible to any JS running in the WebView. If a user switches servers, the old server's refresh token remains in localStorage indefinitely. Should clear tokens from `savedServers` on logout, or at minimum clear the refresh_token for the server being logged out of.

## Bugs (4 items)

- [ ] **B1.** `export_burns` in `export.rs` checks sprint ownership (`is_owner_or_root`) but `list_burns` and `get_burn_summary` don't. Any authenticated user can read burn logs for any sprint via `GET /api/sprints/{id}/burns` and `GET /api/sprints/{id}/burn-summary`, but can't export them. Inconsistent access control.

- [ ] **B2.** `BoardView` in `SprintParts.tsx` has keyboard navigation via `onKeyDown` with ArrowLeft/ArrowRight to move cards between columns. The status order is `["backlog", "in_progress", "blocked", "completed"]` but the board columns use `board.todo` (status=backlog), `board.in_progress`, `board.blocked`, `board.done` (status=completed). When pressing ArrowRight from "Todo", it changes status to `"in_progress"` which is correct. But the `Column` component receives `status="backlog"` for Todo — and `changeStatus` calls `PUT /api/tasks/{id}` with the new status. The keyboard nav works, but the `statusOrder` array doesn't include `"active"` which is a valid task status. Tasks with status `"active"` on the board would be in the `todo` column but keyboard nav would try to set them to `"backlog"` (ArrowLeft) which is a no-op since they're already in the todo column.

- [ ] **B3.** `import_tasks_csv` rolls back the entire transaction on DB error (`tx.rollback()`) but continues to use `errors.push()` for validation errors (like invalid due_date) and `continue` to skip rows. If a DB error occurs mid-import, the rollback happens but `created` count still reflects rows inserted before the error. The response would show `created: N` but those N rows were actually rolled back.

- [ ] **B4.** `BurnsView` in `SprintViews.tsx` initializes `taskId` with `tasks[0]?.id ?? 0`. If the sprint has no tasks, `taskId` is 0. The `submit` function checks `!taskId || taskId <= 0` which correctly prevents submission, but the `Select` component shows an empty dropdown with no indication that tasks need to be added first.

## Validation (2 items)

- [ ] **V1.** `create_sprint` validates `start_date` and `end_date` format but `update_sprint` doesn't validate the date format for `start_date` and `end_date` — it only validates ordering. A malformed date string like `"not-a-date"` could be stored via update even though create would reject it.

- [ ] **V2.** `add_time_report` validates `hours > 0` but `log_burn` (sprint burn) allows `points=0, hours=0` — you can log a burn entry with zero points and zero hours. Should require at least one to be positive.

## Code Quality (3 items)

- [ ] **CQ1.** `import_tasks_csv` has a `let mut errors = Vec::new()` that collects validation errors, but on DB error it does `tx.rollback().await.ok(); return Err(internal(...))` — the collected errors are lost. Should include them in the error response.

- [ ] **CQ2.** `get_active_timers` in `timer.rs` builds SQL with string formatting for user/task ID lookups (`format!("SELECT ... WHERE id IN ({})", uph)`). While the values are bound via `.bind()`, the placeholder string construction is repeated in multiple places. Could use a shared helper.

- [ ] **CQ3.** `BoardView` in `SprintParts.tsx` has `Column` defined as a `useCallback` that returns JSX. This is an anti-pattern — components defined inside `useCallback` lose React's component identity on every render, causing unnecessary unmount/remount of the entire column. Should be extracted to a proper component.

## UX (1 item)

- [ ] **UX1.** `BurnsView` shows "No burns logged yet" when the burn log is empty, but doesn't indicate which sprint is selected or provide a link to add tasks if the sprint has none. The burn form silently does nothing when `taskId` is 0.

---

**Total: 12 items**

Priority order: S1 (getFreshToken health check bypass), S2 (token persistence), B3 (CSV import rollback count), V1 (sprint date validation), then remaining items.
