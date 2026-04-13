# Backlog V41 — Full Codebase Audit (2026-04-13)

Scope: Stability, correctness, security, performance, UX, accessibility, code quality.
No new features.

---

## V41-1 [Medium / Bug] `get_task_votes` doesn't check task existence — returns empty instead of 404
**File:** `routes/misc.rs:11-12`
Same pattern as V40-13/14/15. `get_task_votes` returns `[]` for non-existent task IDs.

## V41-2 [Medium / Bug] `get_task_links` doesn't check task existence — returns empty instead of 404
**File:** `routes/misc.rs:173`
Same pattern. Returns `[]` for non-existent task IDs.

## V41-3 [Medium / Bug] `update_user_role` returns 500 instead of 404 for non-existent user
**File:** `routes/admin.rs:update_user_role`
The DB function does `UPDATE ... WHERE id = ?` (silently succeeds) then `get_user(pool, id)` which fails with a generic error. The route maps this to `internal` (500) instead of 404.

## V41-4 [Medium / Bug] `admin_reset_password` silently succeeds for non-existent user
**File:** `routes/admin.rs:33-44`
`update_user_password` does `UPDATE ... WHERE id = ?` which silently succeeds even if the user doesn't exist. No rows_affected check. The admin gets a 204 success response for a non-existent user.

## V41-5 [Low / Accessibility] `Select` component options use `div` with `onClick` but no `role="option"` or keyboard nav
**File:** `gui/src/components/Select.tsx:95`
The dropdown options are `<div onClick={...}>` without `role="option"`, `tabIndex`, or `aria-selected`. Screen readers can't navigate the custom select.

## V41-6 [Low / Accessibility] `AuthScreen` toggle uses `span` with `onClick` but no `role="button"` or `tabIndex`
**File:** `gui/src/components/AuthScreen.tsx:133`
The "Register / Login" toggle is a clickable `<span>` without keyboard accessibility.

## V41-7 [Low / Bug] `TaskContextMenu` backdrop `div` has `onKeyDown` but no `tabIndex` — never receives keyboard events
**File:** `gui/src/components/TaskContextMenu.tsx:54`
The backdrop overlay has `onKeyDown={e => e.key === "Escape" && close()}` but no `tabIndex`, so it can never receive focus or keyboard events. The Escape handler is dead code.

## V41-8 [Low / Code Quality] `pause`/`resume`/`stop` acquire global config lock unnecessarily
**File:** `engine.rs:pause/resume/stop`
These methods lock `self.config` just to create an idle state for the "user not found" fallback. They could use `get_user_config` (which has caching) or just return a default state without locking the global config.

## V41-9 [Low / Bug] `delete_user` doesn't return 404 for non-existent user
**File:** `routes/admin.rs:delete_user`
`db::delete_user` returns an error only for the "last root" case. If the user ID doesn't exist, the DELETE silently succeeds and returns 204.

## V41-10 [Low / Bug] `join_room` doesn't check room existence — returns 500 on non-existent room
**File:** `routes/rooms.rs:join_room`
`db::join_room` will fail with a foreign key error if the room doesn't exist, which maps to `internal` (500) instead of 404.

## V41-11 [Low / Bug] `leave_room` doesn't check membership — silently succeeds if user isn't a member
**File:** `routes/rooms.rs:leave_room`
The DELETE from `room_members` silently succeeds even if the user isn't a member. Should return 404 or 400.

## V41-12 [Low / Code Quality] `SearchQuery` struct missing `utoipa::ToSchema` — can't be registered in OpenAPI
**File:** `routes/tasks.rs`
`SearchQuery` derives only `Deserialize`, not `ToSchema`. It's a query param so utoipa handles it via `IntoParams`, but it should derive `utoipa::IntoParams` for proper OpenAPI documentation.

---

## Summary

| ID | Severity | Category | Status |
|----|----------|----------|--------|
| V41-1 | Medium | Bug | |
| V41-2 | Medium | Bug | |
| V41-3 | Medium | Bug | |
| V41-4 | Medium | Bug | |
| V41-5 | Low | Accessibility | FALSE POSITIVE — Select already uses button with role="option" |
| V41-6 | Low | Accessibility | FALSE POSITIVE — AuthScreen already uses button element |
| V41-7 | Low | Bug | |
| V41-8 | Low | Code quality | |
| V41-9 | Low | Bug | |
| V41-10 | Low | Bug | |
| V41-11 | Low | Bug | |
| V41-12 | Low | Code quality | |

**Total: 12 items** — 4 medium, 8 low
