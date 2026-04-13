# Backlog V47 — Full Codebase Audit (2026-04-13)

Scope: Stability, correctness, security, performance, UX, accessibility, code quality.
No new features.

---

## V47-1 [Medium / Bug] `delete_webhook` returns 500 instead of 404 for non-existent webhook
**File:** `routes/webhooks.rs:delete_webhook`
The DB function already checks rows_affected, but the route maps the error to `internal` (500) instead of 404.

## V47-2 [Medium / Bug] `get_dependencies` doesn't check task existence — returns empty instead of 404
**File:** `routes/dependencies.rs:get_dependencies`
Returns `[]` for non-existent task IDs.

## V47-3 [Medium / Bug] `get_task_labels` doesn't check task existence — returns empty instead of 404
**File:** `routes/labels.rs:get_task_labels`
Returns `[]` for non-existent task IDs.

## V47-4 [Low / Bug] `add_task_label` returns 500 instead of 404 for non-existent label
**File:** `routes/labels.rs:add_task_label`
If `label_id` doesn't exist, the FK constraint error maps to `internal` (500). Should verify label exists first.

## V47-5 [Low / Bug] `add_dependency` doesn't verify `depends_on` task exists
**File:** `routes/dependencies.rs:add_dependency`
If `depends_on` task doesn't exist, the FK error maps to a generic BAD_REQUEST with the raw error string. Should check existence first with a clear message.

---

## Summary

| ID | Severity | Category | Status |
|----|----------|----------|--------|
| V47-1 | Medium | Bug | FIXED — returns 404 |
| V47-2 | Medium | Bug | FIXED — returns 404 for non-existent task |
| V47-3 | Medium | Bug | FIXED — returns 404 for non-existent task |
| V47-4 | Low | Bug | FIXED — verifies label exists |
| V47-5 | Low | Bug | FIXED — verifies depends_on task exists |

**Total: 5 items** — 5 fixed
