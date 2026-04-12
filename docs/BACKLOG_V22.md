# BACKLOG v22 — Fresh Codebase Audit (2026-04-12)

Full audit of 58 backend .rs files (~7000 LOC), 66 frontend .ts/.tsx files (~9500 LOC), 275 backend tests, 154 frontend tests.

## Security (3 items)

- [x] **S1.** `TaskAttachments` upload in `TaskDetailParts.tsx` uses raw `fetch()` with `Bearer ${token}` from store instead of `apiCall`/`invoke`. This bypasses Tauri's HTTP client and the automatic token refresh logic. If the token expires mid-session, uploads will silently fail with 401 instead of auto-refreshing. Same issue with `AuthImage` download.
  **FIXED** (b8b60e1) — Added `getFreshToken()` to api.ts. AuthImage, upload, and download now verify token validity before fetch().

- [ ] **S2.** `useSseConnection` creates `EventSource` directly from browser using `${url}/api/timer/sse?ticket=...`. This bypasses Tauri's HTTP client. While SSE uses ticket auth (not JWT in URL), the connection goes through the browser's network stack instead of Tauri's, which could behave differently with proxies/CORS.
  **WON'T FIX** — Tauri doesn't support SSE through IPC. Browser EventSource is the correct approach. Ticket-based auth avoids JWT exposure.

- [ ] **S3.** `accept_estimate` in `rooms.rs` updates task fields (`estimated_hours` or `estimated`) based on `estimation_unit` but only handles `"points"` and `"hours"`. Rooms created with `"mandays"` or `"tshirt"` units will have `accept_estimate` store the value in the wrong field or not at all.
  **FALSE POSITIVE** — `accept_estimate` already handles all 4 units: `"hours"` → estimated_hours, `"mandays"` → hours×8, default (points/tshirt) → estimated + remaining_points.

## Bugs (5 items)

- [ ] **B1.** `list_tasks` endpoint (`GET /api/tasks`) has `_claims` parameter (unused) — it doesn't filter by user. Any authenticated user can see ALL tasks. The `export_tasks` endpoint correctly filters by `user_id` for non-root users, but `list_tasks` doesn't.
  **WON'T FIX** — By design. The app is a shared workspace where all authenticated users see all tasks. `export_tasks` filters for data export privacy only.

- [ ] **B2.** `get_task_detail`, `get_task_sessions`, `list_comments`, etc. — none check task ownership. Any authenticated user can read any task's details.
  **WON'T FIX** — Same as B1. Shared workspace model. All read endpoints are intentionally open to authenticated users.

- [x] **B3.** `import_tasks_csv` doesn't validate `due_date` format. Invalid dates from CSV are inserted directly into the DB.
  **FIXED** (189117b) — CSV import now validates due_date with `valid_date()`. Invalid dates are reported as errors and the row is skipped.

- [x] **B4.** `bulk_update_status` doesn't fire webhooks for task updates. Single `update_task` dispatches `task.updated` webhook, but bulk status change skips webhook dispatch entirely.
  **FIXED** (bf2538f) — Dispatches `task.updated` webhook with task IDs, status, and `bulk: true` flag.

- [ ] **B5.** `useRoomWebSocket` creates `WebSocket` directly from browser. The WebSocket bypasses Tauri's HTTP client.
  **WON'T FIX** — Same as S2. Tauri doesn't support WebSocket through IPC. Browser WebSocket with ticket auth is the correct approach.

## Business Logic (3 items)

- [ ] **BL1.** `auto_archive` uses `updated_at < cutoff` to find tasks to archive. A task completed 100 days ago but with a recent comment won't be archived.
  **WON'T FIX** — Safer behavior. Tasks with recent activity shouldn't be auto-archived. A dedicated `completed_at` column would require migration + backfill for marginal benefit.

- [ ] **BL2.** `snapshot_sprint` counts `remaining_points` and `estimated_hours` from task fields, not burn-log values. The burndown shows estimate-based progress.
  **WON'T FIX** — Standard sprint burndown behavior. Burndowns track remaining work (estimates), not effort spent (burns). This is the correct agile pattern.

- [ ] **BL3.** `accept_estimate` auto-advance filters by `t.status != "estimated"` but no task ever gets status `"estimated"` through normal flow.
  **FALSE POSITIVE** — `accept_estimate` in `db/rooms.rs` DOES set task status to `"estimated"` via `update_task(..., Some("estimated"), ...)`. The filter correctly skips already-estimated tasks.

## Validation (2 items)

- [x] **V1.** `import_tasks_csv` doesn't validate `due_date` format from CSV data.
  **FIXED** (189117b) — Same as B3. Duplicate entry.

- [ ] **V2.** `create_room` accepts `estimation_unit` values `"mandays"` and `"tshirt"` but `accept_estimate` only handles `"points"` and `"hours"`.
  **FALSE POSITIVE** — Same as S3. `accept_estimate` handles all 4 units correctly.

## Performance (2 items)

- [ ] **P1.** `get_tasks_full` ETag computation runs a single query with 7 subqueries.
  **WON'T FIX** — Single round-trip, acceptable performance. Optimizing further adds complexity for minimal gain.

- [ ] **P2.** `NotificationBell` polls `/api/notifications/unread` every 30 seconds even when SSE is connected.
  **WON'T FIX** — SSE change events don't carry notification-specific data. Polling is simpler and more reliable for unread counts.

## Accessibility (2 items)

- [x] **A1.** `TaskList` table view has no `<caption>` element describing the table content.
  **FIXED** (bf2538f) — Added screen-reader-only caption with current sort column.

- [ ] **A2.** Sprint board drag-and-drop has no keyboard alternative for moving cards between columns.
  **WON'T FIX** — Significant feature addition. Users can change task status via the task detail view or context menu as keyboard alternatives.

## Code Quality (3 items)

- [x] **CQ1.** `RoomMember` type in frontend `types.ts` has `user_id` field missing.
  **FIXED** (bf2538f) — Added `user_id: number` to match backend struct.

- [ ] **CQ2.** Sprint endpoints return empty results (200 OK) for non-existent sprint IDs instead of 404.
  **WON'T FIX** — Valid REST pattern. Empty collection is a valid response.

- [x] **CQ3.** `TaskDetailView` has unnecessary `export { ExportButton }` re-export.
  **FIXED** (bf2538f) — Removed. ExportButton is already exported from TaskDetailHelpers.tsx.

---

**Total: 20 items**
- **7 fixed:** S1, B3, B4, V1, A1, CQ1, CQ3
- **3 false positive:** S3, BL3, V2
- **10 won't fix:** S2, B1, B2, B5, BL1, BL2, P1, P2, A2, CQ2
