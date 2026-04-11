# API Changelog

## v6 (current)

### Breaking Changes
- `matchSearch` now uses plain substring by default. Regex requires `/pattern/` syntax.
- JWT tokens now include `iat` (issued-at) claim. Old tokens without `iat` still work.
- Refresh token endpoint re-fetches user from DB (role changes take effect immediately).

### New Endpoints
- `GET /api/health` — Health check (no auth required)

### Security
- Rate limiter prefers `x-real-ip` over `x-forwarded-for`
- Webhook dispatch rejects private/loopback IPs (SSRF protection)
- Token blocklist uses RwLock for better read concurrency
- Security headers on all responses (X-Content-Type-Options, X-Frame-Options, Referrer-Policy)
- CSV export prefixes formula-triggering characters
- Swagger UI controllable via `POMODORO_SWAGGER` env var
- Root password configurable via `POMODORO_ROOT_PASSWORD` env var

### Validation
- Task title max 500 chars, description max 10000, project max 200, tags max 500
- Comment content max 10000 chars
- Sprint goal max 1000, retro_notes max 10000
- CSV import max 1MB, priority clamped to 1-5
- Burn log: points max 1000, hours max 24
- Circular parent_id references detected and rejected

### Bug Fixes
- Bulk status uses shared validator (consistent status values)
- Sprint date validation uses NaiveDate parser (not just length check)
- delete_task wrapped in transaction (atomic)
- reorder_tasks verifies ownership of all tasks
- cast_vote verifies room membership
- Monthly recurrence preserves original day
- Due-date reminders reset on date change (not arbitrary count)
- History CSV export properly escapes fields and revokes blob URL

### Performance
- Bulk status: single UPDATE (was N+1)
- reorder_tasks: wrapped in transaction
- recover_interrupted: single UPDATE
- count_tasks: handles assignee and team_id filters
- SprintView.load(): parallel API calls with Promise.all
- New indexes: tasks(user_id), comments(task_id), rooms(creator_id)
