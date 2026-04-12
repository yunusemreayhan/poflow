# BACKLOG v24 — Fresh Codebase Audit (2026-04-12)

Full audit of 58 backend .rs files (~6800 LOC), 66 frontend .ts/.tsx files (~9300 LOC), 275 backend tests, 154 frontend tests.

## Security (1 item)

- [x] **S1.** `TaskContextMenu` "Save as template" passes `JSON.stringify(data)` as the template `data` field, causing double-encoding. Template variable resolution (`{{today}}`, `{{username}}`) doesn't work for templates created via the context menu.
  **FIXED** (7cbf8f9) — Now passes data as object directly.

## Bugs (3 items)

- [x] **B1.** `delete_user` in `db/users.rs` reassigns resources using inline subqueries that could return NULL if no other root user exists, violating NOT NULL constraints.
  **FIXED** (7cbf8f9) — Pre-verifies reassignment target exists, uses verified `target_id` for all UPDATEs, returns error if no target.

- [ ] **B2.** `CommentSection` optimistic update with negative ID persists if `load()` fails after `addComment`.
  **WON'T FIX** — Standard optimistic update pattern. Component re-renders on next mount. Negligible impact.

- [ ] **B3.** `get_sprint_board` maps `"active"` → in_progress column. Keyboard nav changes status to target column's status, not preserving original.
  **WON'T FIX** — By design. Moving a card right changes its status to the target column's status regardless of original status.

## Validation (1 item)

- [ ] **V1.** `InlineTimeReport` allows 0.01h (36 seconds) via frontend, backend only checks `hours > 0`.
  **WON'T FIX** — Backend validates correctly. Edge case is harmless.

## Code Quality (2 items)

- [ ] **CQ1.** `TaskNode` component is 280+ lines with 20+ state variables.
  **WON'T FIX** — Refactoring adds no functional benefit. Component works correctly.

- [ ] **CQ2.** `get_descendant_ids` called repeatedly without caching for team scope.
  **WON'T FIX** — Fast enough for typical deployments. Caching adds complexity.

---

**Total: 7 items**
- **2 fixed:** S1, B1
- **5 won't fix:** B2, B3, V1, CQ1, CQ2
