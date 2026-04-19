# Changelog

## v5 (in progress)

### Security (10/10 — COMPLETE)
- S1: Proper HMAC-SHA256 for webhook signatures (was SHA256(secret+body))
- S2: Auth key derived from random per-installation salt (32 bytes)
- S3: AES-256-GCM encryption for stored auth (was XOR cipher)
- S4: Block executable file extensions in Tauri write_file
- S5: Configurable CORS origins via POFLOW_CORS_ORIGINS env var
- S6/B6: SSE ticket generation fails hard without /dev/urandom
- S7: Bcrypt cost upgrade on login (rehash if cost < 12)
- S8: Random storage key for attachments (was predictable timestamp)
- S9: CSP + X-Content-Type-Options headers on attachment downloads
- S10: Normalize IPv4-mapped IPv6 in rate limiter

### Bugs (14/18)
- B1/P1: User config cache with 60s TTL (tick() no longer does per-second DB queries)
- B2: pause/resume/stop no longer acquire config lock unnecessarily
- B3: get_state() lock ordering fixed (config → states)
- B4: skip() increments session_count when skipping work phase
- B7: WebSocket no longer auto-joins room on connect
- B8: O(n) leaf detection in accept_estimate (was O(n²))
- B9: Non-root users only export their own tasks
- B10: Export sessions accepts from/to date range params
- B11: Removed dead SSE ?token= fallback in frontend
- B12: Module-level _tasksLoadedAt variable (was window cast)
- B14: SSE connect guarded against unmounted component
- B16: doReveal checks room status before calling reveal API
- B17: BurnsView validates taskId > 0 before submit

### Authorization (6/6 — COMPLETE)
- Z1-Z6: Team/epic/sprint operations require admin or root

### Validation (8/8 — COMPLETE)
- V1-V7: Burn task, webhook URL, room type, team role/name, epic name, task existence

### Features
- F9: SSE reconnect with exponential backoff (1s→30s max)
- F10: Bulk status change endpoint (PUT /api/tasks/bulk-status)
- F12: WebSocket keepalive ping every 30s
- F13: Export burns as CSV

### Performance
- P1: User config cache with 60s TTL
- P2: get_state() only refreshes daily_completed on idle

### Code Quality
- Q1: TaskDetailView split (686→572 lines, extracted TaskDetailParts.tsx)
- Q3: Settings split (519→410 lines, extracted SettingsParts.tsx)
- Q4/Q5: Shared validate_username/validate_password in types.rs
- Q7: Zero compiler warnings
- Q9: CSV escape handles carriage returns
- Q10: Date validation uses chrono::NaiveDate

### UX
- U5: Heatmap legend (Less → More gradient)
- U6: Loading skeleton for sprint detail
- U9: Empty state message for sprints list

### Accessibility
- A4: NumInput has aria-label
- A5: Focus trap in confirmation dialog
- A6: Error toasts use role="alert"
- A9: MotionConfig reducedMotion="user"

### Documentation
- D1-D3: Rate limiting, webhook payload, production deployment docs
- D4: Swagger UI endpoint documented
- D5: Environment variables reference

### i18n
- I1: 6 empty state i18n keys (en + tr)
- I4: Keyboard shortcuts panel i18n (7 keys)

## v4 (in progress)

### Security
- JWT secret file permissions set to 0600
- Token type field in JWT Claims (access vs refresh separation)
- Rate limiting on refresh endpoint
- Removed legacy SSE ?token= query parameter (ticket-only)
- Rate limiter fallback for missing IP headers
- Improved SSRF check (169.254, .local, 172.16-31 range)
- SHA-256 for token blocklist and webhook HMAC (was DefaultHasher)
- JWT secret fails hard without /dev/urandom (no weak fallback)
- Config/DB file permissions 0600 on Unix
- Tauri write_file uses async I/O
- Tauri api_call sanitizes error responses
- Auth key derivation uses SHA-256 (32-byte key)
- Non-HTTPS connection warning
- Username validation in profile update
- Password max length 128 chars

### Bug Fixes
- tick() uses per-user config instead of global config
- skip() advances to next phase instead of stopping
- cancel_burn validates sprint_id matches
- SSE notifications for leave_room, start_voting, assignees, comments
- retro_notes textarea updates on SSE push
- stop_session logs errors instead of silently ignoring
- Sprint delete confirmation dialog
- SSE debounce timer cleared on unmount
- Attachment upload/download uses store auth (was broken globals)
- customAccept NaN validation
- AdminPanel null-checks API response
- TeamManager uses /api/users (was /api/admin/users)
- team_id filter included in task count query
- daily_completed counter refreshed from DB on get_state
- doReveal countdown guarded against unmounted component
- Move up/down uses Promise.all (no race condition)
- BurnsView taskId updates when tasks populate

### Authorization & Validation
- Sprint task add/remove requires sprint owner
- Config bounds validation (timer durations, daily goal)
- Sprint/room name validation (non-empty, max 200)
- Estimation unit validation (points/hours/mandays)
- Vote value range validation (0-1000)
- Non-negative burn points/hours
- Positive time report hours
- Non-empty comment content
- estimation_mode validated (hours/points)

### Features
- Break duration display on timer buttons
- Password visibility toggle on auth screen
- Leave room button for non-admin members
- Sprint start/complete confirmation dialogs
- Task picker dropdown in Timer view
- Sprint audit logging (create/start/complete)
- Webhook dispatch for sprint events
- Webhook retry (3 attempts, exponential backoff)
- Sprint scope indicator in burn form
- Shared reqwest client for webhooks
- Atomic config file writes
- Template form builder (structured UI)
- Turkish locale
- String interpolation and pluralization helpers

### Accessibility
- Context menu: Escape key, ARIA roles (menu/menuitem)
- Toggle: role=switch, aria-checked
- Timer buttons: aria-labels
- Auth form: aria-labels, error role=alert
- Sprint list: keyboard accessible (role=button, tabIndex, Enter)

### Code Quality
- PRIORITY_COLORS extracted to shared constants
- Status constants with TypeScript union types
- Removed unused imports and dead code
- Module-level variables instead of (window as any) globals
- Snapshot errors logged instead of silently ignored
- CommentSection extracted to own file
- useSseDebounce custom hook (replaces duplicated pattern)
- Consistent HTTP status codes (room endpoints → 204)
- Null-check apiCall results before setState

---

## v3

### Highlights
- 78 items completed across bugs, security, features, performance,
  code quality, UX, accessibility, tests, and documentation
- JWT refresh tokens with rotation
- File attachments (upload/download/delete)
- i18n framework with English locale
- Task archiving, velocity charts, sprint board drag-and-drop
- Component splitting (TaskContextMenu, SprintViews)
- 97 backend tests, 44 frontend tests

---

## v2

### Highlights
- 61 items completed
- Estimation rooms with planning poker
- Sprint burndown charts and burn tracking
- Epic groups and team management
- Task dependencies and labels
- Recurrence rules
- Webhook system
- CSV/JSON export

---

## v1

### Initial Release
- Multi-user Poflow timer with Rust/axum backend
- Tauri v2 desktop app (React/TypeScript)
- SQLite database with WAL mode
- Hierarchical tasks with drag-and-drop
- Sprint management
- Real-time SSE updates
- JWT authentication
