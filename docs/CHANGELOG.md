# Changelog

## v2.0.0 — Feature Release (2026-04-12)

### New API Endpoints

**Analytics & Insights:**
- `GET /api/analytics/estimation-accuracy` — Estimation accuracy report with per-project breakdown
- `GET /api/analytics/focus-score` — Personal focus score (0-100) with streak tracking
- `GET /api/suggestions/priorities` — Auto-prioritization suggestions based on due dates and staleness
- `GET /api/suggestions/schedule` — Smart scheduling based on historical session patterns
- `GET /api/leaderboard?period=week|month|year` — Team focus leaderboard
- `GET /api/reports/weekly-digest` — Weekly summary report

**Activity & Social:**
- `GET /api/feed?types=audit,comment&since=...&limit=50` — Unified activity feed
- `GET /api/users/presence` — User online status and last activity
- `GET /api/achievements` — List all achievement types with unlock status
- `POST /api/achievements/check` — Check and unlock new achievements

**Integrations:**
- `POST /api/integrations/github` — GitHub webhook receiver (links commits to tasks via `#123` or `task-123` in messages). Set `GITHUB_WEBHOOK_SECRET` env var for HMAC verification.
- `POST /api/integrations/slack` — Register Slack/Discord webhook URL
- `GET/POST /api/tasks/{id}/links` — Task external links (commits, PRs, URLs)

**Automation:**
- `GET/POST /api/automations` — CRUD for automation rules
- `DELETE /api/automations/{id}` — Delete a rule
- `PUT /api/automations/{id}/toggle` — Enable/disable a rule
- Valid triggers: `task.status_changed`, `task.due_approaching`, `task.all_subtasks_done`

**Collaboration:**
- `POST /api/timer/join/{session_id}` — Join another user's active timer session
- `GET /api/timer/participants/{session_id}` — List session participants
- `GET /api/sprints/{id}/retro-report` — Sprint retrospective analytics

**Export:**
- `GET /api/export/ical` — iCal feed (.ics) with tasks and sprints

### Task Enhancements
- PERT estimates: `estimate_optimistic` and `estimate_pessimistic` fields on tasks
- Threaded comments: `parent_id` field on comments for reply chains
- Task checklists: `- [ ]` / `- [x]` in descriptions rendered as interactive checkboxes

### Frontend Features
- Calendar view (month grid with tasks by due date)
- Kanban board (drag-and-drop, grouping by project/user)
- Focus heatmap (365-day GitHub-style)
- Productivity trends (weekly comparison)
- Focus score widget (circular progress)
- Achievements badges
- Mobile responsive (bottom tab bar on small screens)
- PWA support (manifest.json, service worker)
- Offline mode (IndexedDB cache + sync queue)

### Database Migrations
- v11: achievements table
- v12: task_links table
- v13: comment parent_id
- v14: PERT estimate columns
- v15: automation_rules table
- v16: session_participants table

## Error Responses

All endpoints return errors in this format:

```json
{"error": "Human-readable error message"}
```

| Status | Meaning |
|--------|---------|
| 400 | Bad request — validation failed, missing fields, invalid values |
| 401 | Unauthorized — missing or expired JWT token |
| 403 | Forbidden — insufficient permissions (not owner, not root) |
| 404 | Not found — resource doesn't exist |
| 409 | Conflict — duplicate resource (username, label name) |
| 413 | Payload too large — attachment exceeds 10MB |
| 429 | Too many requests — rate limit exceeded |
| 500 | Internal server error — unexpected failure |

## Developer Setup

### Prerequisites
- Rust 1.75+ with `cargo`
- Node.js 18+ with `npm`
- SQLite 3.35+ (for FTS5 support)

### Backend
```bash
cd crates/pomodoro-daemon
cargo check                    # Verify compilation
cargo test                     # Run 310+ tests
cargo run                      # Start server on :9090
```

Environment variables:
- `POMODORO_DATA_DIR` — Database directory (default: `~/.local/share/pomodoro`)
- `POMODORO_ROOT_PASSWORD` — Initial root user password (auto-generated if unset)
- `GITHUB_WEBHOOK_SECRET` — HMAC secret for GitHub webhook verification
- `POMODORO_NO_RATE_LIMIT=1` — Disable auth rate limiting (for tests)

### Frontend
```bash
cd gui
npm install
npm run dev                    # Vite dev server
npx tsc --noEmit               # Type check
npm test                       # Run 154 tests
```

### Database
SQLite with WAL mode. 16 migrations run automatically on startup.
Database file: `$POMODORO_DATA_DIR/pomodoro.db` (default: `~/.local/share/pomodoro/pomodoro.db`)
