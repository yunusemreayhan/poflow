# Changelog

## [0.3.0] - 2026-04-11

### Security
- SSE uses ticket exchange instead of JWT in URL query string
- Logout now calls server to revoke JWT token (persistent blocklist in SQLite)
- Request body size limited to 2MB
- Webhook URLs validated against SSRF (private IPs blocked)
- Default bind address changed from 0.0.0.0 to 127.0.0.1
- SSE tickets use /dev/urandom for cryptographic randomness
- CORS allows x-requested-with header for CSRF defense-in-depth
- CORS exposes pagination headers (x-total-count, x-page, x-per-page)
- JWT secret fallback uses multiple entropy sources with timing jitter
- X-Requested-With header added to all Tauri API calls

### Fixed
- Timer burn log used wrong duration (next phase instead of completed session)
- Webhooks now actually fire HTTP POST on task events
- Recurring tasks now processed every 5 minutes (was dead code)
- Notification preferences (notify_desktop) now respected
- Duplicate recover_interrupted functions consolidated
- `setActiveTab` → `setTab` type mismatch in keyboard handler
- Bulk delete uses app's confirm dialog instead of browser native
- Toast ID collision fixed with random component
- daily_completed midnight reset simplified (reset to 0, DB refreshes on next start)
- Room WS auto-joins user as member
- Duplicate .with_state() removed from router

### Added
- Webhook dispatch on task.created, task.updated, task.deleted
- Recurring task background job (clones template, advances next_due)
- Tests for labels, dependencies, recurrence, webhooks, audit, export, reorder, velocity, logout, password complexity (10 new tests, 82 total)
- aria-live regions on toast notifications and bulk action toolbar
- Labels UI: LabelManager in Settings + TaskLabelPicker in task detail
- Dependencies UI: TaskDependencies component in task detail
- Recurrence UI: TaskRecurrence component in task detail
- Audit log UI: AuditLog component in Settings (admin only)
- Confirm dialog: role="dialog", aria-modal, auto-focus cancel button
- token_blocklist SQLite table for persistent JWT revocation

### Changed
- Silently ignored errors (.ok()) replaced with tracing::warn in engine tick
- Bare unwrap() in route handlers replaced with proper error propagation
- routes/mod.rs split: 25+ request/response structs moved to routes/types.rs
- db/mod.rs split: 27 type definitions moved to db/types.rs
- get_room_state parallelized with tokio::join! (4 concurrent queries)
- TaskList.tsx reduced from 770 to 739 lines (inline editors extracted)
- Global error toast for mutation failures in apiCall
- VoteResult frontend type aligned with backend RoomVote struct
- Config lock in tick() acquired once before states lock (was per-user)
- OpenAPI annotations: 64 → 111 endpoints annotated (near-complete coverage)
- Sessions export endpoint (GET /api/export/sessions?format=csv|json)
- Auth tokens stored in Tauri filesystem backend instead of localStorage (XSS mitigation)
- Loading progress bar indicator in main content area
- Bulk checkboxes become visible when any are selected (discoverability)
- ETag for tasks/full now includes sprint_tasks, burns, and assignees counts
- Room WS + ticket exchange test (83 backend tests total)
- Frontend tests for matchSearch, countDescendants (22 frontend tests total)
- Architecture doc updated with v0.3.0 changes (webhook dispatch, recurrence, token blocklist)
- Task templates: CRUD API (GET/POST/DELETE /api/templates) with JSON template data
- Due date reminders: background job checks every 30min, desktop notification for overdue/due-tomorrow
- Sprint retro notes: retro_notes field on sprints (nullable text, editable via PUT)
- Notification sound: XDG sound theme "complete" played on session end (controlled by notify_sound preference)
- Connection pool: 4 connections (was 2), busy_timeout 5s, min_connections 1
- Frontend bundle splitting: vendor/motion/icons manual chunks in Vite
- CLI expansion: sprints, labels, label, deps, export subcommands
- Search UX: clear button + result count in search bar
- Drag-and-drop: opacity feedback on dragged item

## [0.2.0] - 2026-04-11

### Security
- Auto-generated JWT secret (persisted to `~/.local/share/pomodoro/.jwt_secret`)
- Restrictive CSP in Tauri webview
- CORS restricted to localhost/tauri origins
- Rate limiting on auth endpoints (10 req/60s per IP)
- Bcrypt operations moved to `spawn_blocking` (non-blocking async)
- Username validation (alphanumeric, max 32 chars)
- `write_file` Tauri command restricted to safe directories
- Input validation on task creation/update (title, priority, estimated, status)
- Role validation on admin and room role endpoints

### Fixed
- **Multi-user timer**: Each user now has independent timer state
- **CLI**: Rewritten to use HTTP API (was broken with Unix socket)
- **Burndown snapshots**: Use `estimated` points for total (was using `remaining_points`)
- **History**: Added `user_id` filter parameter
- **Daily completed**: Now per-user instead of global
- **Timestamps**: Standardized to millisecond precision with `now_str()` helper
- **Epic snapshots**: Fixed SQLite CAST issue for REAL columns
- **Room auto-advance**: Uses vote history to skip already-estimated tasks
- **Room task loading**: Limited instead of loading all tasks

### Performance
- `get_task_detail`: Batch CTE query (was N+1 recursive)
- `get_descendant_ids`: Single recursive CTE (was iterative N queries)
- `delete_task`: Batch cascade with CTE (was N queries per child)
- SQLite pool reduced to 2 connections (was 5)
- Added index on `burn_log.user_id`

### Added
- Task search API: `?search=`, `?assignee=`, `?due_before=`, `?due_after=`, `?priority=`
- Structured JSON error responses (`{"error":"...","code":"..."}`)
- Loading state tracking in frontend store
- Skip-to-content link, ARIA labels, focus indicators
- `prefers-reduced-motion` support
- ARIA roles on custom Select component
- 10 new integration tests (Teams, Epics, Sprint Roots, User Config, ETag, Profile, etc.)

### Changed
- `db.rs` split into 11 submodules (`db/users.rs`, `db/tasks.rs`, etc.)
- `routes.rs` split into 16 submodules (`routes/auth_routes.rs`, `routes/tasks.rs`, etc.)
- Error toast auto-dismiss increased to 6s (was 3s)
- User config overlay deduplicated via `engine.get_user_config()`

## [0.1.0] - 2026-04-10

### Added
- Initial release
- Pomodoro timer with configurable durations
- Hierarchical task management
- Sprint management with Kanban board and burndown charts
- Estimation rooms (planning poker)
- Multi-user JWT authentication
- Burn log (unified time & point tracking)
- Teams and Epic Groups
- Tauri v2 desktop GUI
- CLI client
- systemd user service
- .deb packaging
