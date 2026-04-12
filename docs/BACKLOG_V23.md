# BACKLOG v23 — Fresh Codebase Audit (2026-04-12)

Full audit of 58 backend .rs files (~6800 LOC), 66 frontend .ts/.tsx files (~9300 LOC), 275 backend tests, 154 frontend tests.

## Security (2 items)

- [x] **S1.** `getFreshToken()` in `api.ts` calls `/api/health` to verify the token is valid before returning it. But `/api/health` doesn't require auth — it always returns 200. The health check doesn't actually verify the token.
  **FIXED** (5a1cc1b) — Now uses authenticated `GET /api/timer` instead.

- [x] **S2.** `savedServers` in `store.ts` stores tokens (including refresh tokens) in `localStorage` via `saveServers()`. These persist across sessions and are accessible to any JS running in the WebView. Old server's refresh token remains in localStorage indefinitely after logout.
  **FIXED** (5a1cc1b) — Logout now removes the current server's entry from savedServers.

## Bugs (4 items)

- [ ] **B1.** `export_burns` checks sprint ownership but `list_burns` and `get_burn_summary` don't. Inconsistent access control.
  **WON'T FIX** — Consistent with shared workspace model (v22 B1/B2). Sprint read endpoints are intentionally open. Export has ownership check for data export privacy.

- [ ] **B2.** `BoardView` keyboard nav status order doesn't include `"active"` status.
  **WON'T FIX** — Sprint board uses backlog/in_progress/blocked/completed. "active" maps to "todo" on the board. By design.

- [x] **B3.** `import_tasks_csv` rollback count mismatch — response shows `created: N` but rows were rolled back on DB error.
  **FIXED** (5a1cc1b) — Error message now includes rollback info.

- [ ] **B4.** `BurnsView` initializes `taskId` with 0 when sprint has no tasks. Form silently does nothing.
  **WON'T FIX** — Submit correctly checks `taskId <= 0`. Negligible UX impact.

## Validation (2 items)

- [x] **V1.** `update_sprint` doesn't validate `start_date`/`end_date` format — only validates ordering. Malformed dates could be stored.
  **FIXED** (5a1cc1b) — Added YYYY-MM-DD format validation matching `create_sprint`.

- [x] **V2.** `log_burn` allows `points=0, hours=0` — empty burn entries.
  **FIXED** (5a1cc1b) — Now requires at least one of points or hours to be positive.

## Code Quality (3 items)

- [x] **CQ1.** `import_tasks_csv` collected errors are lost on DB rollback.
  **FIXED** (5a1cc1b) — Same as B3. Duplicate entry.

- [ ] **CQ2.** Repeated SQL placeholder construction pattern across multiple endpoints.
  **WON'T FIX** — Common pattern, extracting a helper adds complexity for minimal benefit.

- [ ] **CQ3.** `BoardView` Column defined inside `useCallback` — anti-pattern.
  **WON'T FIX** — Performance impact negligible for 4 columns. Dependencies are stable.

## UX (1 item)

- [ ] **UX1.** `BurnsView` empty state doesn't indicate which sprint is selected or guide user to add tasks.
  **WON'T FIX** — Same as B4. Negligible impact.

---

**Total: 12 items**
- **6 fixed:** S1, S2, B3, V1, V2, CQ1
- **6 won't fix:** B1, B2, B4, CQ2, CQ3, UX1
