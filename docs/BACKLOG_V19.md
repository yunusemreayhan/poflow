# BACKLOG v19 — Fresh Codebase Audit (2026-04-12)

Full audit of 56 backend .rs files (6547 LOC), 53 frontend .ts/.tsx files (9282 LOC), 275 backend tests, 154 frontend tests.

## Security (5 items)

- [x] **S1.** `api_rate_limit` middleware uses `std::sync::Mutex` (blocking) inside async context — replaced with `parking_lot::Mutex` + sliding window counter.
- [x] **S2.** `RateLimiter` uses `Vec<Instant>` per IP — O(n) cleanup per request. Replaced with sliding window counter (two counters per window), O(1) per request.
- [x] **S3.** `Sidebar` sends partial config `{ theme: th }` — now merges with current config before PUT to prevent overwriting all settings.
- [x] **S4.** Webhook secret encryption uses XOR — replaced with AES-256-GCM via `aes-gcm` crate. Legacy XOR decryption preserved as fallback.
- [x] **S5.** `download_attachment` reads entire file into memory — now streams via `tokio_util::io::ReaderStream`.

## Bugs (12 items)

- [x] **B1.** Theme toggle resets all config — fixed (same as S3, merges with current config).
- [x] ~~**B2.**~~ FALSE POSITIVE — SSE gap on token refresh is covered by 2-second polling fallback.
- [x] **B3.** Duplicate `import { useStore }` in Rooms.tsx — removed.
- [x] **B4.** `TeamManager` calls `/api/users` expecting `{id, username}[]` but got `string[]` — changed endpoint to return `{id, username}` objects.
- [x] **B5.** CSV import doesn't handle new export format — now detects columns by header name, supports both simple and full export formats.
- [x] ~~**B6.**~~ FALSE POSITIVE — `duplicate_task` already copies `work_duration_minutes`.
- [x] **B7.** `carryover_sprint` copies old dates — now leaves dates blank for user to set.
- [x] ~~**B8.**~~ WON'T FIX — ErrorBoundary zustand usage is unconventional but functional.
- [x] **B9.** Same root cause as B4 — fixed by changing `/api/users` return type.
- [x] ~~**B10.**~~ FALSE POSITIVE — `change_password` already returns 401 UNAUTHORIZED.
- [x] ~~**B11.**~~ WON'T FIX — restore_backup dead pool is inherent limitation, note tells user to restart.
- [x] **B12.** `isDescendantOf` O(depth × n) — now uses Map for O(depth) lookup.

## Business Logic (6 items)

- [x] **BL1.** Assigned user can now unassign themselves (not just task owner/root). Updated test.
- [x] **BL2.** Team admins can now delete their own teams (not just root).
- [x] ~~**BL3.**~~ FALSE POSITIVE — error message already clear ("Use /start or /complete endpoints").
- [x] ~~**BL4.**~~ FALSE POSITIVE — auto_archive only targets completed tasks, not active timer tasks.
- [x] ~~**BL5.**~~ WON'T FIX — T-shirt estimate display is a frontend mapping concern.
- [x] **BL6.** Carryover sprint now filters out tasks already in an active sprint.

## Validation (4 items)

- [x] ~~**V1.**~~ FALSE POSITIVE — `validate_username` already enforces alphanumeric+_- and max 32 chars.
- [x] ~~**V2.**~~ FALSE POSITIVE — `create_task` already validates title length (max 500).
- [x] **V3.** `update_sprint` now validates project length (max 200) on update.
- [x] ~~**V4.**~~ WON'T FIX — daily hours limit is a nice-to-have, not critical.

## UX Improvements (6 items)

- [x] ~~**UX1.**~~ WON'T FIX — Dashboard widget enhancement is a feature request.
- [x] ~~**UX2.**~~ FALSE POSITIVE — History already renders task_path breadcrumbs.
- [x] ~~**UX3.**~~ WON'T FIX — Blocked task indicators in task list is a feature request.
- [x] ~~**UX4.**~~ WON'T FIX — Label editing is a feature request.
- [x] ~~**UX5.**~~ WON'T FIX — Webhook editing is a feature request.
- [x] **UX6.** Move up/down now offsets sort_order by ±1 when values are equal.

## Accessibility (3 items)

- [x] **A1.** NotificationBell dropdown now has focus trap (Tab cycles within dialog).
- [x] **A2.** CSV import label is now keyboard-focusable and activatable with Enter/Space.
- [x] ~~**A3.**~~ FALSE POSITIVE — Rooms toggle already has `aria-pressed`.

## Performance (3 items)

- [x] ~~**P1.**~~ FALSE POSITIVE — `get_task_detail` already batch-loads comments/sessions.
- [x] ~~**P2.**~~ WON'T FIX — Teams caching is premature optimization.
- [x] ~~**P3.**~~ WON'T FIX — Client-side ETag comparison is negligible overhead.

## Code Quality (4 items)

- [x] **CQ1.** Duplicate import removed (same as B3).
- [x] **CQ2.** Blocking mutex replaced (same as S1).
- [x] ~~**CQ3.**~~ WON'T FIX — Webhook dispatch queue is premature optimization (6 call sites).
- [x] ~~**CQ4.**~~ WON'T FIX — Inline SQL extraction is code style preference.

---

**Total: 43 items** — 20 FIXED, 12 FALSE POSITIVE, 11 WON'T FIX

### Commits
1. `799a361` — B1/S3, B4/B9, B5, S4, B7 (theme config, team API, CSV, AES-GCM, carryover)
2. `3c748c8` — S1/S2, S5, B3/CQ1, B12, V3, BL1, BL2, BL6 (rate limiter, streaming, assignees, teams)
3. `c0e8909` — A1, A2 (notification focus trap, CSV keyboard)
4. `702c332` — UX6 (sort_order swap fix)
