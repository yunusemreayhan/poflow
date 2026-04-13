# Backlog V45 — Full Codebase Audit (2026-04-13)

Scope: Stability, correctness, security, performance, UX, accessibility, code quality.
No new features.

---

## V45-1 [Medium / Bug] `get_epic_group` returns 500 instead of 404 for non-existent epic group
**File:** `routes/epics.rs:26-28`
`db::get_epic_group_detail(...).map_err(internal)` — maps the "not found" error to 500 instead of 404.

## V45-2 [Medium / Bug] `add_sprint_root_tasks` doesn't validate `task_ids` length or emptiness
**File:** `routes/epics.rs:89-99`
Unlike `add_epic_group_tasks` (which validates `is_empty()` and `len() > 500`), `add_sprint_root_tasks` accepts any number of task IDs including empty arrays and unbounded lists.

## V45-3 [Medium / Bug] `add_sprint_root_tasks` doesn't batch-validate task existence
**File:** `routes/epics.rs:89-99`
Unlike `add_epic_group_tasks` (which batch-validates all task IDs exist), `add_sprint_root_tasks` only catches FK errors per-task. This means it partially succeeds — some tasks get added before a non-existent one fails.

## V45-4 [Low / Code Quality] `add_sprint_root_tasks` doesn't deduplicate task_ids
**File:** `routes/epics.rs:89-99`
Unlike `add_epic_group_tasks` (which deduplicates), `add_sprint_root_tasks` passes duplicates through. The DB uses `INSERT OR IGNORE` so it's harmless, but inconsistent.

## V45-5 [Low / Bug] `list_epic_groups` doesn't filter by user — all users see all epic groups
**File:** `routes/epics.rs:list_epic_groups`
Unlike rooms (which filter by membership for non-root), epic groups are visible to all authenticated users regardless of ownership.

---

## Summary

| ID | Severity | Category | Status |
|----|----------|----------|--------|
| V45-1 | Medium | Bug | FIXED — returns 404 |
| V45-2 | Medium | Bug | FIXED — validates length and emptiness |
| V45-3 | Medium | Bug | FIXED — batch-validates existence |
| V45-4 | Low | Code quality | FIXED — deduplicates task_ids |
| V45-5 | Low | Bug | WON'T FIX — epic groups are intentionally shared |

**Total: 5 items** — 4 fixed, 1 won't fix
