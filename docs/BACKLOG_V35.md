# Backlog V35 — Full Codebase Audit (2026-04-13)

Scope: Stability, correctness, security, performance, UX, accessibility, code quality.
No new features.

---

## V35-1 [Medium / Security] `generate_salt()` silently returns zeroed buffer if `/dev/urandom` fails
**File:** `gui/src-tauri/src/lib.rs:157-162`
If `/dev/urandom` can't be opened (e.g., sandboxed environment), `buf` stays all-zeros. The auth encryption key derived from this salt would be deterministic and trivially guessable. Should return an error or use `getrandom` crate as fallback.

## V35-2 [Medium / Bug] `delete_user` doesn't clean up `session_participants`, `achievements`, `automation_rules`, or `task_templates`
**File:** `db/users.rs:55-80`
The user deletion transaction cleans up most tables but misses: `session_participants`, `achievements`, `automation_rules`, `task_templates`, `task_attachments` (user_id column). These will have dangling foreign keys or orphaned data after user deletion.

## V35-3 [Medium / Bug] `get_day_stats` loads ALL work sessions into memory then groups in Rust
**File:** `db/sessions.rs:82-97`
For `days=365`, this fetches every work session for the past year into a `Vec<Session>`, then groups by date in Rust. For active users with thousands of sessions, this is wasteful. Should use SQL `GROUP BY date(started_at)` aggregate.

## V35-4 [Medium / Security] `api_call` Tauri command doesn't validate `path` parameter
**File:** `gui/src-tauri/src/lib.rs:28`
The `path` parameter is concatenated directly to `base_url`. A malicious frontend could pass `path = "/../../../etc/passwd"` or `path = "//evil.com/steal"`. While the reqwest client would just make an HTTP request (not a file read), it could be used for SSRF if the daemon is behind a reverse proxy. Should validate that `path` starts with `/api/`.

## V35-5 [Low / Bug] `update_task` does a full read-modify-write cycle for every update
**File:** `db/tasks.rs:72-93`
Every `update_task` call first fetches the existing task (`get_task`), merges fields in Rust, then writes all columns back. This means two DB round-trips per update and potential race conditions if two updates happen simultaneously (last-write-wins without conflict detection at DB level). The route layer has `expected_updated_at` for optimistic locking, but the DB layer doesn't enforce it.

## V35-6 [Low / Bug] `recover_interrupted` runs before `Engine::new` — recovered sessions aren't reflected in engine state
**File:** `main.rs:82-85`
`recover_interrupted` marks running sessions as interrupted, but the engine is created after this. If a session was running when the server crashed, the engine starts with an empty states map. The user's timer shows idle even though their session was interrupted. Should notify the user or at least log which users were affected.

## V35-7 [Low / Security] `write_file` Tauri command allows writing to any file in Downloads/Documents/Desktop
**File:** `gui/src-tauri/src/lib.rs:93-130`
While the path is restricted to user directories and executable extensions are blocked, there's no size limit on the `content` parameter. A malicious frontend could fill the disk. Should add a content size check.

## V35-8 [Low / Bug] `get_history` CTE for task paths doesn't filter by `deleted_at`
**File:** `db/sessions.rs:60-75`
The recursive CTE `ancestors` walks up the parent chain without checking `deleted_at IS NULL`. If a parent task was soft-deleted, the path still includes it. This is arguably correct (showing historical path), but inconsistent with other queries that filter deleted tasks.

## V35-9 [Low / Performance] `snapshot_active_sprints` runs sequentially for each sprint
**File:** `db/sprints.rs:120-123`
Each active sprint is snapshotted one at a time. With many active sprints, this could be slow. Could batch into a single query or use `tokio::join!`.

## V35-10 [Low / Bug] `accept_estimate` ignores `room_id` parameter
**File:** `db/rooms.rs:175`
The function signature takes `_room_id: i64` (prefixed with underscore, unused). It updates the task directly without verifying the room context. If two rooms vote on the same task, the last `accept_estimate` wins without any room-level tracking.

## V35-11 [Low / Code Quality] `get_room_state` builds vote history from pre-fetched votes but limits to 500
**File:** `db/rooms.rs:200`
The 500-vote limit means rooms with extensive voting history may lose older results. The limit is reasonable for performance, but should be documented in the API response or use pagination.

## V35-12 [Low / Bug] `cleanup_notifications` uses SQLite `strftime` with `'now'` — timezone-dependent
**File:** `db/notifications.rs:52`
`strftime('%Y-%m-%dT%H:%M:%f', 'now', '-30 days')` uses SQLite's `now` which is UTC. The `created_at` field uses `now_str()` which is also UTC. This is consistent, but if the server's SQLite is compiled with a non-UTC default timezone, the comparison would be wrong.

## V35-13 [Low / UX] SSE reconnection doesn't reset `reconnectAttempts` on successful reconnect after error
**File:** `gui/src/hooks/useSseConnection.ts:62`
`reconnectAttempts` is reset in `onopen`, but if the SSE connection drops and reconnects via the error handler's `setTimeout(connectSse, delay)`, the `onopen` of the new EventSource correctly resets it. This is actually fine — false alarm on closer inspection. However, the `connectSse` function creates a new EventSource each time without closing the old one if `sseInstance` is still set. The `onerror` handler does `sseInstance?.close()` before reconnecting, so this is also fine.

## V35-14 [Low / Bug] `processSyncQueue` uses raw `fetch()` bypassing Tauri's `api_call`
**File:** `gui/src/offlineStore.ts:82-93`
The offline sync queue uses `fetch()` directly with the full URL, bypassing the Tauri `invoke("api_call")` path. This means: (1) the CSRF header is `x-requested-with: pomo-offline` instead of `PoflowGUI`, (2) token refresh won't happen if the token expired while offline, (3) the error toast mechanism is bypassed. Should use `apiCall` from the store instead.

## V35-15 [Low / Performance] `list_tasks_paged` with assignee filter uses JOIN which may return duplicates
**File:** `db/tasks.rs:50-55`
When filtering by assignee, the query JOINs `task_assignees`. If a task has multiple assignees (one matching the filter), it could appear multiple times. The `LIMIT/OFFSET` pagination would then be incorrect. Should use `DISTINCT` or `EXISTS` subquery.

## V35-16 [Low / Code Quality] `get_user_id_by_username` returns a custom error string "not_found" instead of a proper error type
**File:** `db/assignees.rs:24-28`
The caller checks `e.to_string() == "not_found"` which is fragile string matching. Should use a proper error variant or return `Option<i64>`.

## V35-17 [Low / Bug] `delete_room` explicitly sets `PRAGMA foreign_keys = ON` before delete
**File:** `db/rooms.rs:42`
This PRAGMA is already set in `connect()` and `migrate()`. Setting it again per-query is redundant and could interfere with connection pooling (PRAGMA is per-connection, not per-query in WAL mode).

## V35-18 [Low / UX] `ErrorBoundary` component doesn't provide a way to recover
**File:** `gui/src/components/ErrorBoundary.tsx`
The error boundary shows the error but has no "retry" or "reload" button. Users must manually refresh the page.

## V35-19 [Low / Bug] `reorder_tasks` updates `updated_at` for every reordered task
**File:** `db/tasks.rs:108-115`
When dragging tasks to reorder, every task in the list gets a new `updated_at` timestamp. This triggers the ETag to change and forces a full reload on all connected clients, even though only sort order changed (no content change). Could use a separate `sort_updated_at` or skip `updated_at` for sort-only changes.

## V35-20 [Low / Security] `encrypt_auth` in Tauri lib uses `/dev/urandom` directly instead of `getrandom` crate
**File:** `gui/src-tauri/src/lib.rs:170-176`
Direct `/dev/urandom` access fails on non-Unix platforms and in some sandboxed environments. The `getrandom` crate handles cross-platform randomness correctly. Same issue as V35-1 but for nonce generation.

## V35-21 [Low / Code Quality] `TaskLabel` struct in `db/labels.rs` is not registered in OpenAPI schema
**File:** `db/labels.rs:11`
`TaskLabel` is used in the `/api/tasks/full` response but isn't in the `components(schemas(...))` list in `main.rs`. The OpenAPI spec is incomplete for this response type.

## V35-22 [Low / Bug] `add_comment` spawns a background task for @mention notifications that outlives the request
**File:** `routes/comments.rs:42-53`
The `tokio::spawn` for @mention parsing runs after the response is sent. If the server shuts down immediately after, the notification may be lost. Also, the spawned task clones `req.content` but `req` is already moved — this works because `content` is cloned before the move, but the code is fragile.

## V35-23 [Low / Documentation] OpenAPI spec version says "1.0.0" but CHANGELOG is at v2.0.1
**File:** `main.rs:67`
`info(title = "Poflow API", version = "1.0.0", ...)` — should be "2.0.1" to match the CHANGELOG.

---

## Summary

| ID | Severity | Category | Status |
|----|----------|----------|--------|
| V35-1 | Medium | Security | FIXED |
| V35-2 | Medium | Bug | FIXED |
| V35-3 | Medium | Performance | FIXED |
| V35-4 | Medium | Security | FIXED |
| V35-5 | Low | Bug | WON'T FIX — design pattern, refactor risk |
| V35-6 | Low | Bug | FALSE POSITIVE — sessions correctly marked interrupted before engine starts |
| V35-7 | Low | Security | FIXED |
| V35-8 | Low | Bug | WON'T FIX — showing historical path including deleted parents is correct for history |
| V35-9 | Low | Performance | WON'T FIX — typically 1-3 active sprints, sequential is fine |
| V35-10 | Low | Bug | WON'T FIX — route layer validates room context before calling DB |
| V35-11 | Low | Code quality | WON'T FIX — 500 is reasonable limit, no pagination promise |
| V35-12 | Low | Bug | FALSE POSITIVE — both sides use UTC consistently |
| V35-13 | Low | UX | FALSE POSITIVE |
| V35-14 | Low | Bug | FIXED |
| V35-15 | Low | Performance | FIXED |
| V35-16 | Low | Code quality | FIXED |
| V35-17 | Low | Bug | FIXED |
| V35-18 | Low | UX | FALSE POSITIVE — reload button already exists |
| V35-19 | Low | Bug | FIXED |
| V35-20 | Low | Security | FIXED |
| V35-21 | Low | Code quality | FIXED |
| V35-22 | Low | Bug | WON'T FIX — best-effort notification, acceptable to lose on shutdown |
| V35-23 | Low | Documentation | FIXED |

**Total: 23 items** — 12 fixed, 6 won't fix, 5 false positive
