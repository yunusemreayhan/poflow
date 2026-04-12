# BACKLOG v20 — Fresh Codebase Audit (2026-04-12)

Full audit of 56 backend .rs files (~6600 LOC), 53 frontend .ts/.tsx files (~9300 LOC), 275 backend tests, 154 frontend tests.

## Security (3 items)

- [x] **S1.** Root config save now preserves bind_address, bind_port, cors_origins from current config — prevents API from overwriting server network settings with serde defaults.
- [x] **S2.** Added password_changed_at column (migration 10). Auth rejects tokens with iat < password_changed_at. admin_reset_password invalidates user cache for immediate effect.
- [x] **S3.** Block dangerous MIME types (HTML, JS, SVG, XML) on upload. Updated test to expect 400.

## Bugs (8 items)

- [x] **B1.** Replaced allTasks.find() with taskMap.get() (useMemo Map) in Sprints.tsx for O(1) lookups.
- [x] **B2.** Same fix in EpicBurndown.tsx.
- [x] **B3.** Fixed optimistic comment ID — use modulo to stay within safe integer range.
- [x] **B4.** Card selection reset now depends on primitive values instead of array reference.
- [x] ~~**B5.**~~ WON'T FIX — scope fetch bounded by task tree depth, inherent to feature.
- [x] **B6.** CSV export now includes work_duration_minutes column.
- [x] **B7.** bulk_update_status now runs auto-unblock logic for dependents when completing tasks.
- [x] ~~**B8.**~~ FALSE POSITIVE — snapshots use ON CONFLICT DO UPDATE, no duplicates created.

## Business Logic (4 items)

- [x] ~~**BL1.**~~ WON'T FIX — label delete permissions requires schema change (adding created_by).
- [x] **BL2.** Sprint task add notifications batched per user — one notification instead of N.
- [x] **BL3.** Carryover sprint now copies goal from completed sprint.
- [x] ~~**BL4.**~~ FALSE POSITIVE — dead `status != "estimated"` filter is harmless.

## Validation (3 items)

- [x] **V1.** Fixed webhook private IP check — enumerate all 172.16-31.x.x ranges individually.
- [x] **V2.** JSON import now counts total tasks including children (max 2000).
- [x] ~~**V3.**~~ FALSE POSITIVE — timestamps are both UTC (now_str uses Utc::now).

## UX Improvements (4 items)

- [x] ~~**UX1.**~~ WON'T FIX — history pagination is a feature request.
- [x] ~~**UX2.**~~ WON'T FIX — move feedback is a feature request.
- [x] ~~**UX3.**~~ WON'T FIX — password dictionary is a feature request.
- [x] ~~**UX4.**~~ WON'T FIX — room reopen is a feature request.

## Accessibility (2 items)

- [x] ~~**A1.**~~ WON'T FIX — aria-grabbed is deprecated in ARIA 1.1; keyboard support already present.
- [x] ~~**A2.**~~ FALSE POSITIVE — already has role="radiogroup" + role="radio" + aria-checked + aria-label.

## Performance (2 items)

- [x] ~~**P1.**~~ WON'T FIX — ETag edge case is theoretical, extremely unlikely.
- [x] **P2.** BoardView dependency fetch skipped when all board tasks are done.

## Code Quality (3 items)

- [x] ~~**CQ1.**~~ WON'T FIX — per-hook client needed for DNS pinning security.
- [x] **CQ2.** Merged into B1/B2 — taskMap useMemo in Sprints, SprintParts, EpicBurndown.
- [x] ~~**CQ3.**~~ WON'T FIX — component size is code style preference.

---

**Total: 29 items** — 16 FIXED, 5 FALSE POSITIVE, 8 WON'T FIX

### Commits
1. `2d35ddc` — S1-S3, B7, V1 (config network, token revocation, MIME block, bulk unblock, webhook IP)
2. `6200299` — B1/B2/CQ2, V2, BL3 (taskMap lookups, JSON import limit, carryover goal)
3. `2edbed0` — B3, B4, BL2, P2 (comment ID, card reset, batch notifications, dep fetch)
4. `8eed742` — B6 (CSV export work_duration_minutes)
