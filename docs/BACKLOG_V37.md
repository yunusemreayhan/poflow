# Backlog V37 — Full Codebase Audit (2026-04-13)

Scope: Stability, correctness, security, performance, UX, accessibility, code quality.
No new features.

---

## V37-1 [Medium / Bug] `cleanup_notifications` still uses SQLite `strftime('now')`
**File:** `db/notifications.rs:55`
`DELETE FROM notifications WHERE read = 1 AND created_at < strftime('%Y-%m-%dT%H:%M:%f', 'now', '-30 days')` — last remaining SQLite `'now'` usage after V36 cleanup. Should compute cutoff in Rust and bind as parameter for consistency.

## V37-2 [Medium / Bug] `encrypt_secret` in webhooks uses `rand::rng()` for nonce instead of `getrandom`
**File:** `db/webhooks.rs:48-49`
`rand::rng().fill(&mut nonce_bytes)` — while `rand` uses `getrandom` internally, this is inconsistent with the rest of the codebase which now uses `getrandom` directly. More importantly, the `encrypt_secret` function calls `.expect("encryption failed")` which would panic on AES failure.

## V37-3 [Medium / Security] `derive_key()` in webhooks falls back to hashing `data_dir` path if no JWT secret exists
**File:** `db/webhooks.rs:28-31`
If neither `POFLOW_JWT_SECRET` env var nor `.jwt_secret` file exists, the encryption key is derived from the data directory path — which is predictable (`~/.local/share/poflow`). Any attacker who knows the data dir can decrypt all webhook secrets.

## V37-4 [Medium / Bug] `config.try_lock()` in `build_router` can fail silently
**File:** `lib.rs:23`
`engine.config.try_lock().map(...)` — `try_lock()` on a tokio `Mutex` returns `Err` if the lock is held. During startup this is unlikely, but if it fails, CORS origins from config are silently dropped. Should use `.lock().await` or handle the error.

## V37-5 [Low / Bug] `edit_comment` endpoint exists in router but has no `#[utoipa::path]` annotation or OpenAPI registration
**File:** `routes/comments.rs`, `main.rs`
The `edit_comment` route is registered in the router (`PUT /api/comments/{id}`) but may be missing from OpenAPI paths. Let me verify.

## V37-6 [Low / Bug] `list_tasks` returns all users' tasks for non-root users
**File:** `routes/tasks.rs:list_tasks`
The `TaskFilter` has `user_id: None` for all users. Non-root users can see all tasks in the system. The `list_deleted_tasks` correctly filters by user, but `list_tasks` doesn't. This may be intentional for team visibility, but inconsistent with `export_tasks` which filters by user.

## V37-7 [Low / Bug] `add_dependency` doesn't check for circular dependencies
**File:** `routes/dependencies.rs:add_dependency`
Adding a dependency doesn't verify that the dependency graph remains acyclic. If task A depends on B and B depends on A, both will be "blocked" forever with no way to unblock.

## V37-8 [Low / Bug] `duplicate_task` copies dependencies but not recurrence
**File:** `routes/tasks.rs:duplicate_task`
Labels, assignees, and dependencies are copied, but recurrence settings are not. If the original task has a recurrence pattern, the copy won't inherit it.

## V37-9 [Low / Security] `webhook.dispatch` retries with exponential backoff but no jitter
**File:** `webhook.rs:dispatch`
`tokio::time::sleep(Duration::from_secs(1 << attempts))` — without jitter, multiple failing webhooks will retry at exactly the same time, causing thundering herd. Should add random jitter.

## V37-10 [Low / Bug] `get_sprint_board` not checking sprint existence before querying
**File:** `routes/teams.rs:get_sprint_board`
If the sprint doesn't exist, the DB function returns an empty board instead of a 404. The route doesn't verify the sprint exists first.

## V37-11 [Low / Code Quality] `UserEntry` struct in `misc.rs` duplicates what could be a DB type
**File:** `routes/misc.rs`
`UserEntry { id, username }` is defined in routes but could be a shared type. Minor duplication.

## V37-12 [Low / Bug] `export_ical` doesn't escape DTEND for sprints — off-by-one day
**File:** `routes/export.rs:export_ical`
iCal `DTEND;VALUE=DATE` is exclusive (the event ends before this date). Sprint end dates should have +1 day added for correct display in calendar apps.

## V37-13 [Low / UX] Timer `prevSessionRef` captures stale session ID on phase change
**File:** `gui/src/components/Timer.tsx:72-73`
`prevSessionRef.current = engine?.current_session_id ?? null` — the `?? null` coercion means `undefined` becomes `null`, but the note prompt uses `prevSessionRef.current` which was set before the phase change. This works correctly because the ref captures the old value before the effect runs, but the `?? null` is misleading since `notePrompt` expects `{ sessionId: number }`.

## V37-14 [Low / Performance] `get_tasks_full` ETag computation queries 7 aggregates in one query
**File:** `routes/misc.rs:get_tasks_full`
The ETag query `SELECT COALESCE((SELECT MAX(...)), ''), (SELECT COUNT(*) FROM tasks...), ...` runs 7 subqueries. This is fine for SQLite but could be slow with large datasets. The ETag approach is correct though.

## V37-15 [Low / Bug] `carryover_sprint` doesn't copy sprint root tasks to the new sprint
**File:** `routes/sprints.rs:carryover_sprint`
When carrying over incomplete tasks, the new sprint gets the tasks but not the root task configuration. If the original sprint had root tasks for scoping, the carry-over sprint won't have the same scope.

## V37-16 [Low / Code Quality] `NotifPref` struct not registered in OpenAPI schemas
**File:** `routes/profile.rs`, `main.rs`
`NotifPref` is used in `get_notif_prefs`/`update_notif_prefs` responses but isn't in the `components(schemas(...))` list.

## V37-17 [Low / Bug] `edit_comment` allows editing any comment content without length validation
**File:** `routes/comments.rs`
Need to verify if `edit_comment` validates content length like `add_comment` does.

## V37-18 [Low / UX] Dashboard `SprintProgress` widget doesn't handle missing end_date
**File:** `gui/src/components/Dashboard.tsx`
If an active sprint has no `end_date`, the progress calculation may produce NaN or incorrect values.

## V37-19 [Low / Code Quality] `BulkStatusRequest` not registered in OpenAPI schemas
**File:** `routes/tasks.rs`, `main.rs`
`BulkStatusRequest` is used by `bulk_update_status` but isn't in the schemas list.

## V37-20 [Low / Bug] `webhook.dispatch` doesn't filter by `slack:` prefix events
**File:** `webhook.rs:dispatch`, `routes/misc.rs:create_slack_integration`
Slack integrations store events as `slack:sprint.started,sprint.completed`. The `get_active_webhooks` query matches on exact event name, so `sprint.started` won't match `slack:sprint.started,...`. Slack webhooks may never fire.

---

## Summary

| ID | Severity | Category | Status |
|----|----------|----------|--------|
| V37-1 | Medium | Bug | FIXED — Rust-side chrono cutoff replaces strftime('now') |
| V37-2 | Medium | Bug | FIXED — getrandom for nonce, Result instead of panic |
| V37-3 | Medium | Security | FIXED — generates random .jwt_secret instead of hashing data_dir |
| V37-4 | Medium | Bug | FIXED — async build_router with lock().await |
| V37-5 | Low | Bug | FALSE POSITIVE — edit_comment is registered in OpenAPI |
| V37-6 | Low | Bug | WON'T FIX — intentional for team visibility, frontend filters via teamScope |
| V37-7 | Low | Bug | FALSE POSITIVE — recursive CTE already checks for circular deps |
| V37-8 | Low | Bug | FIXED — duplicate_task now copies recurrence |
| V37-9 | Low | Security | FIXED — jitter added to webhook retry backoff |
| V37-10 | Low | Bug | FIXED — 404 for non-existent sprint board |
| V37-11 | Low | Code quality | WON'T FIX — minor duplication, refactor not worth it |
| V37-12 | Low | Bug | FIXED — iCal DTEND +1 day per RFC 5545 |
| V37-13 | Low | UX | WON'T FIX — cosmetic, code works correctly |
| V37-14 | Low | Performance | WON'T FIX — acceptable for SQLite |
| V37-15 | Low | Bug | FALSE POSITIVE — root tasks are a team concept, not sprint |
| V37-16 | Low | Code quality | FIXED — NotifPref registered in OpenAPI schemas |
| V37-17 | Low | Bug | FALSE POSITIVE — edit_comment validates length (max 10000) |
| V37-18 | Low | UX | FALSE POSITIVE — endDate null handled correctly |
| V37-19 | Low | Code quality | FIXED — BulkStatusRequest registered in OpenAPI schemas |
| V37-20 | Low | Bug | FIXED — slack: prefix events now matched in dispatch |

**Total: 20 items** — 10 fixed, 4 won't fix, 6 false positive
