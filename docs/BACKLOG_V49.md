# Backlog V49 — Full Codebase Audit (2026-04-13)

Scope: Stability, correctness, security, performance, UX, accessibility, code quality.
No new features.

---

## V49-1 [Medium / Bug] `leave_room` returns 500 instead of 404 for non-existent room
**File:** `routes/rooms.rs:62`
`db::get_room(...).map_err(internal)` — maps "not found" to 500.

## V49-2 [Medium / Bug] `cast_vote` returns 500 instead of 404 for non-existent room
**File:** `routes/rooms.rs:106`
Same pattern — `db::get_room(...).map_err(internal)`.

## V49-3 [Medium / Bug] `accept_estimate` returns 500 instead of 404 for non-existent room
**File:** `routes/rooms.rs:139`
Same pattern.

## V49-4 [Medium / Bug] `get_room_state` returns 500 instead of 404 for non-existent room
**File:** `routes/rooms.rs:36`
`db::get_room_state(...).map_err(internal)` — maps "not found" to 500.

---

## Summary

| ID | Severity | Category | Status |
|----|----------|----------|--------|
| V49-1 | Medium | Bug | |
| V49-2 | Medium | Bug | |
| V49-3 | Medium | Bug | |
| V49-4 | Medium | Bug | |

**Total: 4 items** — 4 medium
