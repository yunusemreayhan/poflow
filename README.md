# Pomodoro Linux

A production-grade multi-user Pomodoro timer and project management platform for Linux. Rust HTTP backend (221 API endpoints), Tauri v2 desktop GUI, web GUI (PWA), hierarchical task management, sprint planning, estimation rooms, Gantt charts, custom workflows, RBAC, and time tracking. Packaged as a single `.deb`.

## Features

### Timer
- Pomodoro work/break cycles with configurable durations
- Auto-start breaks and work sessions
- Desktop notifications on session completion
- **Timer state persistence**: Timer state survives daemon restarts (restored as paused)
- Daily goal tracking

### Hierarchical Task Management
- Unlimited nesting (projects → epics → stories → subtasks)
- Inline create, edit, delete with cascade
- Double-click to rename task titles
- Status tracking: backlog → in_progress → completed
- Priority (1-5), estimated pomodoros, estimated hours, story points
- Task assignees (many-to-many)
- Comments on tasks
- Time reports with auto-assignment
- Recursive rollup of hours, points, and session time
- Export tasks as Markdown, JSON, or XML

### File Attachments
- Upload files to tasks (10MB max per file)
- Download and delete attachments
- Filename sanitization (path traversal protection)

### Labels, Dependencies & Recurrence
- Create/manage labels with custom colors
- Task dependencies (depends-on relationships)
- Recurring tasks (daily/weekly/biweekly/monthly)

### Task Templates
- Save task configurations as reusable templates
- Create/list/delete via API

### Internationalization (i18n)
- Zustand-based locale store with 90+ typed string keys
- English locale included, extensible to any language
- Locale selector in Settings

### Sprint Management
- Create sprints with name, project, goal, and date range
- Sprint lifecycle: planning → active → completed
- **Board tab**: Kanban columns (Todo / In Progress / Done) with click-to-change-status
- **Backlog tab**: Add/remove tasks using the full hierarchical task tree
- **Burndown tab**: SVG line chart with ideal vs actual remaining (toggle points/hours/tasks)
- **Summary tab**: Stats cards, velocity, per-user progress bars
- Auto-snapshot: hourly background task captures burndown data for active sprints
- Sprint badges on all task views (green = active sprint, pale green = past sprint)

### Projects
- First-class project entities with name, description, unique key, lead user, and status
- CRUD via `GET/POST /api/projects`, `GET/PUT/DELETE /api/projects/{id}`
- Tasks, sprints, and rooms can reference a project by `project_id`
- Admin-only creation; project lead can update/delete

### Burn Log (Unified Time & Point Tracking)
- Single source of truth for all burned time and points
- Three sources: `manual` (sprint burns), `timer` (auto-logged on pomodoro completion), `time_report` (ad-hoc hour logging)
- Sprint-scoped burns (optional sprint_id) or task-level burns
- Timer auto-logs hours (duration/3600) with session_id reference on pomodoro completion
- Soft-delete (cancel) with full audit trail — who cancelled what
- Per-user per-day summary view
- Per-task burn totals computed from burn_log
- Cancelled entries remain visible with strikethrough
- `time_reports` table eliminated — replaced by burn_log with source="time_report"

### Estimation Rooms (Planning Poker)
- Create rooms with points or hours estimation
- Real-time voting with card deck (Fibonacci for points, linear for hours)
- 3-2-1 countdown reveal animation

### Custom Statuses & Workflows
- Define custom task statuses beyond the built-in set (e.g., `code_review`, `qa_testing`, `deployed`)
- Each status has a category (`todo`, `in_progress`, `done`) for sprint board column mapping
- Sprint board automatically maps custom statuses to the correct column
- CRUD via `GET/POST /api/statuses`, `PUT/DELETE /api/statuses/{id}`
- **Workflow transition rules**: Define allowed status transitions per project (e.g., `backlog` → `active` → `in_progress` → `done`)
- Enforced on task update — returns 422 if transition is not allowed
- CRUD via `GET/POST /api/workflows/transitions`, `DELETE /api/workflows/transitions/{id}`

### Custom Fields
- Define custom fields on tasks: text, number, select, date, user types
- Select fields support predefined options
- Set/get/delete values per task
- Custom field values included in task detail response
- Filter tasks by custom field values via advanced search

### Task Checklists
- Lightweight sub-items on tasks (simpler than full subtasks)
- Toggle checked state, reorder, add/remove
- Assignees can manage checklists on tasks assigned to them

### RBAC (Role-Based Access Control)
- Three-tier role system: `root` > `admin` > `user`
- **Admin** can manage all tasks, sprints, labels, statuses, fields, reports
- **Admin** cannot manage users, system config, or backups (root only)
- **User** can only manage their own tasks (plus tasks assigned to them)

### Bulk Operations
- `PUT /api/tasks/bulk-status` — change status of multiple tasks
- `POST /api/tasks/bulk-assign` — assign a user to multiple tasks
- `POST /api/tasks/bulk-sprint` — move multiple tasks to a sprint

### Advanced Search
- `POST /api/tasks/search/advanced` — structured query with JSON body
- Filter by: status (eq/neq/in), project, assignee, label, priority (gt/gte/lt), due_date, title (contains), custom fields (`custom:field_name`)
- Sort by: priority, due_date, created_at, updated_at, title
- Pagination via limit/offset
- **Saved views**: Save and restore custom task filters and sort orders

### Time Tracking Reports
- `GET /api/reports/time-tracking` — hours per user per project per week
- CSV export via `?format=csv`
- Date range filtering via `?from=&to=`

### Web GUI
- The daemon serves the React frontend as static files
- Access the full app at `http://server:9090` in any browser
- No Tauri desktop app required — works on any device
- Platform abstraction layer: Tauri IPC in desktop mode, fetch() in web mode

### Gantt Chart
- SVG timeline showing tasks with due dates as horizontal bars
- Dependency arrows between linked tasks
- Zoom in/out (4 levels), navigate by weeks
- Today line, weekend shading, overdue highlighting
- Status-colored bars

### Roadmap View
- Epic-level overview with progress bars
- Task count, completion percentage, story points
- Status breakdown segments per epic

### Automation Rules
- User-defined trigger→action rules executed automatically
- Triggers: `task.status_changed`, `task.all_subtasks_done`
- Conditions: filter by from/to status
- Actions: `set_status`, `set_priority`
- Auto-complete parent when all subtasks done

### Multi-User
- JWT authentication (bcrypt + 7-day tokens)
- First registered user becomes root (auto-generated password saved to `~/.local/share/pomodoro/.root_password`, or set `POMODORO_ROOT_PASSWORD` env var)
- Root users can manage all users and override ownership
- Everyone sees all data; ownership controls edit/delete
- Profile management (change username/password)
- Admin panel for user role management

### Webhooks & Integrations
- Create/manage webhooks with SSRF protection (blocks private/loopback addresses)
- HMAC-SHA256 signature verification for GitHub push webhooks
- Slack and Discord webhook support
- GitHub/GitLab commit auto-linking (parses #123 / task-123 from commit messages)
- **Webhook delivery logs**: Track webhook dispatch history with status and response

### Notifications & Watchers
- In-app notification system with per-event-type preferences
- Watch tasks for change notifications
- Due date reminders (30-minute check interval)
- Unread count badge

### Automations
- User-defined automation rules with triggers and actions
- Triggers: task status changed, due approaching, all subtasks done

### Analytics & Reports
- Focus score and estimation accuracy analytics
- Leaderboard (weekly/monthly/all-time)
- Activity feed and weekly digest
- Per-user hours report
- Schedule and priority suggestions
- Achievement system with unlock tracking

### Admin & Security
- Database backup and restore
- Full audit log of all user actions
- JWT with refresh token rotation and token revocation
- Rate limiting and CSRF validation
- **Read rate limiting**: GET endpoints limited to 1000 req/min per IP (mutations: 200/min)
- Auto-archive completed tasks (configurable days)

### Architecture
- **Backend**: Rust + axum HTTP server on port 9090
- **Frontend**: Tauri v2 + React + TypeScript + Tailwind v4
- **Database**: SQLite with foreign key constraints
- **Auth**: JWT with user_id-based identity (usernames are changeable)
- **API**: OpenAPI/Swagger UI at `/swagger-ui/`
- **State**: Zustand store with Tauri invoke → reqwest bridge

## Database Schema (37 tables)

All user references use `user_id INTEGER REFERENCES users(id)` — usernames are resolved via JOINs. This means usernames can be changed without breaking any foreign key relationships.

| Table | Purpose |
|---|---|
| `users` | id, username (unique, changeable), password_hash, role, created_at |
| `tasks` | Hierarchical tasks with user_id FK, parent_id self-ref, status, estimates |
| `sessions` | Pomodoro timer sessions with user_id FK |
| `session_participants` | Multi-user session participation tracking |
| `comments` | Comments on tasks with user_id FK |
| `task_assignees` | Many-to-many task↔user with user_id FK |
| `task_dependencies` | Task dependency relationships (depends-on) |
| `task_labels` | Many-to-many task↔label mapping |
| `task_links` | External links (commits, PRs, issues, docs) on tasks |
| `task_recurrence` | Recurring task patterns (daily/weekly/biweekly/monthly) |
| `task_templates` | Reusable task configuration templates |
| `task_watchers` | Users watching tasks for notifications |
| `task_attachments` | File attachments on tasks |
| `labels` | Label definitions with name and color |
| `rooms` | Estimation rooms with creator_id FK |
| `room_members` | Room membership with user_id FK and role |
| `room_votes` | Votes with user_id FK, unique per room+task+user |
| `sprints` | Sprint metadata with created_by_id FK |
| `sprint_tasks` | Sprint↔task mapping with added_by_id FK |
| `sprint_daily_stats` | Burndown snapshots per sprint per day |
| `sprint_root_tasks` | Root task scoping for sprints |
| `burn_log` | Unified burn tracking: manual entries, timer completions, time reports |
| `user_configs` | Per-user timer configuration overrides |
| `teams` | Team definitions with name |
| `team_members` | Team↔user membership with roles |
| `team_root_tasks` | Root task scoping for teams |
| `epic_groups` | Epic group definitions for cross-sprint tracking |
| `epic_group_tasks` | Epic group↔task mapping |
| `epic_snapshots` | Daily burndown snapshots for epic groups |
| `achievements` | User achievement tracking |
| `automation_rules` | User-defined automation rules (triggers + actions) |
| `audit_log` | Full audit trail of user actions |
| `notifications` | In-app notification queue |
| `notification_prefs` | Per-user notification preferences per event type |
| `webhooks` | User webhook configurations for external integrations |
| `token_blocklist` | Revoked JWT tokens |
| `schema_migrations` | Database migration version tracking |

## REST API

### Auth (no JWT required)
| Method | Endpoint | Description |
|---|---|---|
| POST | `/api/auth/register` | Register new user |
| POST | `/api/auth/login` | Login, returns JWT with user_id |

### Timer
| Method | Endpoint | Description |
|---|---|---|
| GET | `/api/timer` | Get timer state |
| POST | `/api/timer/start` | Start timer (task_id, phase) |
| POST | `/api/timer/pause` | Pause |
| POST | `/api/timer/resume` | Resume |
| POST | `/api/timer/stop` | Stop |
| POST | `/api/timer/skip` | Skip current phase |

### Tasks
| Method | Endpoint | Description |
|---|---|---|
| GET | `/api/tasks` | List all tasks |
| POST | `/api/tasks` | Create task |
| GET | `/api/tasks/{id}` | Get task detail (recursive) |
| PUT | `/api/tasks/{id}` | Update task (owner/root) |
| DELETE | `/api/tasks/{id}` | Delete task with cascade (owner/root) |

### Comments, Burns, Assignees
| Method | Endpoint | Description |
|---|---|---|
| GET/POST | `/api/tasks/{id}/comments` | List/add comments |
| DELETE | `/api/comments/{id}` | Delete comment |
| GET/POST | `/api/tasks/{id}/time` | List/add burns for task (time_report source) |
| GET | `/api/tasks/{id}/burn-total` | Aggregated burned points+hours for task |
| GET/POST | `/api/tasks/{id}/assignees` | List/add assignees |
| DELETE | `/api/tasks/{id}/assignees/{username}` | Remove assignee |
| GET | `/api/tasks/{id}/votes` | Get estimation votes for task |
| GET | `/api/task-sprints` | Get all task↔sprint mappings |

### Estimation Rooms
| Method | Endpoint | Description |
|---|---|---|
| GET/POST | `/api/rooms` | List/create rooms |
| GET/DELETE | `/api/rooms/{id}` | Get state (auto-joins) / delete |
| POST | `/api/rooms/{id}/join` | Join room |
| POST | `/api/rooms/{id}/leave` | Leave room |
| DELETE | `/api/rooms/{id}/members/{username}` | Kick member (admin) |
| PUT | `/api/rooms/{id}/role` | Set member role (admin) |
| POST | `/api/rooms/{id}/start-voting` | Start voting on task (admin) |
| POST | `/api/rooms/{id}/vote` | Cast vote |
| POST | `/api/rooms/{id}/reveal` | Reveal votes (admin) |
| POST | `/api/rooms/{id}/accept` | Accept estimate + auto-advance (admin) |
| POST | `/api/rooms/{id}/close` | Close room (admin) |

### Sprints
| Method | Endpoint | Description |
|---|---|---|
| GET/POST | `/api/sprints` | List (filter: ?status=&project=) / create |
| GET/PUT/DELETE | `/api/sprints/{id}` | Detail / update / delete |
| POST | `/api/sprints/{id}/start` | Start sprint (→ active + snapshot) |
| POST | `/api/sprints/{id}/complete` | Complete sprint (snapshot + → completed) |
| GET/POST | `/api/sprints/{id}/tasks` | List / add tasks (bulk) |
| DELETE | `/api/sprints/{id}/tasks/{tid}` | Remove task from sprint |
| GET | `/api/sprints/{id}/burndown` | Get burndown data |
| POST | `/api/sprints/{id}/snapshot` | Manual burndown snapshot |
| GET | `/api/sprints/{id}/board` | Kanban board (todo/wip/done) |

### Burn Log
| Method | Endpoint | Description |
|---|---|---|
| POST | `/api/sprints/{id}/burn` | Log a burn (task_id, points, hours, note) |
| GET | `/api/sprints/{id}/burns` | List all burns (including cancelled) |
| DELETE | `/api/sprints/{id}/burns/{bid}` | Cancel a burn (soft-delete) |
| GET | `/api/sprints/{id}/burn-summary` | Per-user per-day aggregated totals |

### Admin & Profile
| Method | Endpoint | Description |
|---|---|---|
| GET | `/api/admin/users` | List users (root only) |
| PUT | `/api/admin/users/{id}/role` | Change user role (root only) |
| DELETE | `/api/admin/users/{id}` | Delete user (root only) |
| PUT | `/api/profile` | Update own username/password |
| GET/PUT | `/api/config` | Get/update timer config |
| GET | `/api/history` | Session history (filter: ?from=&to=&user_id=) |
| GET | `/api/stats` | Day stats (filter: ?days=) |

### Tasks (search/filter)
| Parameter | Description |
|---|---|
| `?search=` | Search title and tags (LIKE match) |
| `?assignee=` | Filter by assigned username |
| `?due_before=` | Tasks due before date |
| `?due_after=` | Tasks due after date |
| `?priority=` | Filter by priority (1-5) |
| `?page=&per_page=` | Pagination (default: page 1, 5000 per page) |
| `?team_id=` | Filter by team scope |

### Sprint Roots & Scope
| Method | Endpoint | Description |
|---|---|---|
| GET/POST | `/api/sprints/{id}/roots` | List/add root tasks for sprint scoping |
| DELETE | `/api/sprints/{id}/roots/{tid}` | Remove root task |
| GET | `/api/sprints/{id}/scope` | Get all descendant task IDs from roots |
| GET | `/api/sprints/burndown` | Global burndown (all active sprints) |

### Teams
| Method | Endpoint | Description |
|---|---|---|
| GET/POST | `/api/teams` | List/create teams |
| GET/DELETE | `/api/teams/{id}` | Get detail / delete (root only) |
| POST | `/api/teams/{id}/members` | Add member |
| DELETE | `/api/teams/{id}/members/{uid}` | Remove member |
| POST | `/api/teams/{id}/roots` | Add root tasks |
| DELETE | `/api/teams/{id}/roots/{tid}` | Remove root task |
| GET | `/api/teams/{id}/scope` | Get all descendant task IDs |
| GET | `/api/me/teams` | Get current user's teams |

### Epic Groups
| Method | Endpoint | Description |
|---|---|---|
| GET/POST | `/api/epics` | List/create epic groups |
| GET/DELETE | `/api/epics/{id}` | Get detail / delete |
| POST | `/api/epics/{id}/tasks` | Add tasks to group |
| DELETE | `/api/epics/{id}/tasks/{tid}` | Remove task |
| POST | `/api/epics/{id}/snapshot` | Manual burndown snapshot |

### Batch & Real-time
| Method | Endpoint | Description |
|---|---|---|
| GET | `/api/tasks/full` | Batch: tasks + sprints + burns + assignees (ETag support) |
| GET | `/api/users` | List all usernames |
| GET | `/api/burn-totals` | All task burn totals |
| GET | `/api/assignees` | All task assignees |
| GET | `/api/timer/sse?token=` | Server-Sent Events for timer + data changes |

### Labels
| Method | Endpoint | Description |
|---|---|---|
| GET/POST | `/api/labels` | List/create labels |
| DELETE | `/api/labels/{id}` | Delete label |
| GET | `/api/tasks/{id}/labels` | Get task's labels |
| PUT | `/api/tasks/{id}/labels/{label_id}` | Add label to task |
| DELETE | `/api/tasks/{id}/labels/{label_id}` | Remove label from task |

### Dependencies
| Method | Endpoint | Description |
|---|---|---|
| GET/POST | `/api/tasks/{id}/dependencies` | List/add dependencies |
| DELETE | `/api/tasks/{id}/dependencies/{dep_id}` | Remove dependency |
| GET | `/api/dependencies` | Get all task dependencies |

### Recurrence
| Method | Endpoint | Description |
|---|---|---|
| GET/PUT/DELETE | `/api/tasks/{id}/recurrence` | Get/set/remove recurrence pattern |

### Webhooks
| Method | Endpoint | Description |
|---|---|---|
| GET/POST | `/api/webhooks` | List/create webhooks |
| PUT | `/api/webhooks/{id}` | Update webhook |
| DELETE | `/api/webhooks/{id}` | Delete webhook |

### Audit & Export
| Method | Endpoint | Description |
|---|---|---|
| GET | `/api/audit` | Query audit log (?entity_type=&entity_id=&page=&per_page=) |
| GET | `/api/export/tasks` | Export tasks (?format=csv or json) |
| GET | `/api/export/sessions` | Export sessions (?format=csv or json, ?from=&to=) |
| GET | `/api/export/burns/{sprint_id}` | Export sprint burns as CSV |
| GET | `/api/export/ical` | Export tasks and sprints as iCal feed |
| POST | `/api/import/tasks` | Import tasks from CSV |
| POST | `/api/import/tasks/json` | Import tasks from JSON (with hierarchy) |
| POST | `/api/auth/logout` | Revoke current JWT token |

### Attachments
| Method | Endpoint | Description |
|---|---|---|
| GET | `/api/tasks/{id}/attachments` | List task attachments |
| POST | `/api/tasks/{id}/attachments` | Upload attachment (10MB max) |
| GET | `/api/attachments/{id}/download` | Download attachment |
| DELETE | `/api/attachments/{id}` | Delete attachment |

### Notifications
| Method | Endpoint | Description |
|---|---|---|
| GET | `/api/notifications` | List notifications (?limit=) |
| GET | `/api/notifications/unread` | Get unread count |
| POST | `/api/notifications/read` | Mark notifications read |
| GET | `/api/profile/notifications` | Get notification preferences |
| PUT | `/api/profile/notifications` | Update notification preferences |

### Automations
| Method | Endpoint | Description |
|---|---|---|
| GET | `/api/automations` | List automation rules |
| POST | `/api/automations` | Create automation rule |
| DELETE | `/api/automations/{id}` | Delete automation rule |
| PUT | `/api/automations/{id}/toggle` | Toggle automation enabled/disabled |

### Integrations
| Method | Endpoint | Description |
|---|---|---|
| POST | `/api/integrations/github` | GitHub/GitLab push webhook receiver |
| POST | `/api/integrations/slack` | Create Slack/Discord webhook integration |

### Analytics & Reports
| Method | Endpoint | Description |
|---|---|---|
| GET | `/api/analytics/focus-score` | User focus score analytics |
| GET | `/api/analytics/estimation-accuracy` | Estimation accuracy report |
| GET | `/api/leaderboard` | Leaderboard (?period=week/month/all) |
| GET | `/api/feed` | Activity feed |
| GET | `/api/reports/weekly-digest` | Weekly digest report |
| GET | `/api/reports/user-hours` | Per-user hours report |
| GET | `/api/suggestions/schedule` | AI schedule suggestions |
| GET | `/api/suggestions/priorities` | AI priority suggestions |
| GET | `/api/achievements` | List user achievements |
| POST | `/api/achievements/check` | Check/unlock achievements |

### Templates
| Method | Endpoint | Description |
|---|---|---|
| GET | `/api/templates` | List task templates |
| POST | `/api/templates` | Create template |
| GET | `/api/templates/{id}` | Get template |
| DELETE | `/api/templates/{id}` | Delete template |
| POST | `/api/templates/{id}/instantiate` | Create task from template |

### Watchers
| Method | Endpoint | Description |
|---|---|---|
| POST | `/api/tasks/{id}/watch` | Watch a task |
| DELETE | `/api/tasks/{id}/watch` | Unwatch a task |
| GET | `/api/tasks/{id}/watchers` | List task watchers |
| GET | `/api/watched` | List all watched tasks |

### Task Operations
| Method | Endpoint | Description |
|---|---|---|
| POST | `/api/tasks/{id}/duplicate` | Duplicate a task |
| POST | `/api/tasks/bulk-status` | Bulk update task status |
| PUT | `/api/tasks/reorder` | Reorder tasks (sort_order) |
| GET | `/api/tasks/search` | Search tasks |
| GET | `/api/tasks/trash` | List soft-deleted tasks |
| POST | `/api/tasks/{id}/restore` | Restore soft-deleted task |
| DELETE | `/api/tasks/{id}/permanent` | Permanently delete task |
| GET | `/api/tasks/{id}/sessions` | List task sessions |
| GET | `/api/tasks/{id}/time-summary` | Task time summary |
| GET | `/api/tasks/{id}/links` | List task links |
| POST | `/api/tasks/{id}/links` | Add task link |
| PUT | `/api/sessions/{id}/note` | Add note to session |

### Timer (additional)
| Method | Endpoint | Description |
|---|---|---|
| GET | `/api/timer/active` | List active timers |
| POST | `/api/timer/join/{session_id}` | Join a session |
| GET | `/api/timer/participants/{session_id}` | List session participants |
| POST | `/api/timer/ticket` | Create SSE auth ticket |

### Admin (additional)
| Method | Endpoint | Description |
|---|---|---|
| PUT | `/api/admin/users/{id}/password` | Reset user password (root) |
| POST | `/api/admin/backup` | Create database backup |
| GET | `/api/admin/backups` | List backups |
| POST | `/api/admin/restore` | Restore from backup |
| POST | `/api/auth/refresh` | Refresh JWT token |
| POST | `/api/auth/password` | Change password |

### Sprints (additional)
| Method | Endpoint | Description |
|---|---|---|
| POST | `/api/sprints/{id}/carryover` | Carry over incomplete tasks to new sprint |
| GET | `/api/sprints/{id}/retro-report` | Sprint retrospective report |
| GET | `/api/sprints/compare` | Compare two sprints |
| GET | `/api/sprints/velocity` | Velocity chart data |

### Presence
| Method | Endpoint | Description |
|---|---|---|
| GET | `/api/users/presence` | User presence (online/last active) |
| GET | `/api/rooms/{id}/export` | Export room voting results |
| GET | `/api/health` | Health check |

### Projects
| Method | Endpoint | Description |
|---|---|---|
| GET/POST | `/api/projects` | List/create projects |
| GET/PUT/DELETE | `/api/projects/{id}` | Get/update/delete project |

### Workflow Transitions
| Method | Endpoint | Description |
|---|---|---|
| GET/POST | `/api/workflows/transitions` | List/create transition rules (?project=) |
| DELETE | `/api/workflows/transitions/{id}` | Delete transition rule |

### Saved Views
| Method | Endpoint | Description |
|---|---|---|
| GET/POST | `/api/views` | List/create saved views |
| PUT/DELETE | `/api/views/{id}` | Update/delete saved view |

## Installation

### Docker (recommended)

```bash
# Quick start
docker compose up -d

# Or build and run manually
docker build -t pomodoro .
docker run -d -p 9090:9090 -v pomodoro-data:/data pomodoro

# Open http://localhost:9090
```

### From source (Ubuntu/Debian)

The install script handles everything: dependency checks, building, and installing.

```bash
# Server + web GUI + CLI (most users)
./install.sh

# Server + web GUI + CLI + Tauri desktop app
./install.sh --desktop

# Just install system dependencies (useful for CI)
./install.sh --deps-only
```

After install:
```bash
systemctl --user daemon-reload
systemctl --user enable --now pomodoro
# Web GUI: http://localhost:9090
# Desktop: pomodoro-gui
# CLI: pomo --help
```

**Prerequisites:** Rust (via rustup), Node.js 20+, npm. For `--desktop`: `cargo install tauri-cli`.

**System packages** (auto-installed by the script):
- Base: `build-essential pkg-config libssl-dev libsqlite3-dev`
- Desktop only: `libwebkit2gtk-4.1-dev libgtk-3-dev libappindicator3-dev librsvg2-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev`

### .deb package (server + web GUI only)

```bash
cd gui && npm ci && npx vite build && cd ..
cargo deb -p pomodoro-daemon
sudo dpkg -i target/debian/pomodoro-daemon_*.deb
```

### ⚠️ Desktop GUI build note

The desktop app **must** be built with `cargo tauri build`, not `cargo build -p pomodoro-gui`. Regular `cargo build` produces a dev binary that tries to connect to `localhost:1420` (Vite dev server) instead of using the embedded frontend. The install script handles this automatically with `--desktop`.

## Testing

### ⚠️ Run tests before pushing

```bash
# Run all 3 quality gates at once
./check.sh

# Or individually:
# 1. Unit/integration tests (fast, no GUI needed)
cargo test -p pomodoro-daemon

# 2. Frontend unit tests
cd gui && npm test

# 3. E2E GUI tests (requires built binaries + display)
./e2etests/run_e2e.sh
```

All three gates must pass before pushing to main.

### Unit & Integration Tests

520 backend tests run automatically (`cargo test -p pomodoro-daemon`):

```bash
cargo test -p pomodoro-daemon
```

Tests use in-memory SQLite — no disk I/O, fully isolated, no port conflicts.

### Frontend Unit Tests

209 frontend tests across 17 test files (`cd gui && npm test`):

```bash
cd gui && npm test
```

Tests cover store logic, i18n, utils, tree operations, rollup, and error boundary.

### E2E GUI Tests

887 end-to-end tests across 46 files drive the real Tauri GUI via WebDriver against an isolated daemon. 100% API endpoint coverage (221/221 endpoints tested).

```bash
# Run all E2E tests
./e2etests/run_e2e.sh

# Run a specific test file
./e2etests/run_e2e.sh test_flows.py

# Run a specific test class
./e2etests/run_e2e.sh test_flows.py::TestLogin
```

**Coverage areas:**
- GUI flows: login, registration, timer, task detail, sprint board, settings, theme, sidebar, keyboard shortcuts
- API exhaustive: every endpoint (221/221), every status transition, every config field, pagination, search
- Security: JWT tampering, IDOR, privilege escalation, rate limiting, SQL injection, path traversal
- Edge cases: unicode/emoji, 10K-char strings, HTML injection, boundary values, input validation
- Data integrity: lifecycle counts, sprint column invariants, dependency chains, import/export round-trips
- Performance: startup time, API latency, memory usage, concurrent throughput, P99 latency
- Multi-user: permissions, cross-user collaboration, team workflows, estimation rooms
- WebSocket: ticket auth, room state push, vote updates
- Stress: 500 concurrent task creates, 200 rapid requests, concurrent burns/votes
- Idempotency: double-delete, double-start, double-vote safety
- Workflow scenarios: 10 realistic end-to-end user stories

**Writing new tests:** See [`e2etests/CHEATSHEET.md`](e2etests/CHEATSHEET.md) for 12 copy-paste patterns and [`e2etests/helpers.py`](e2etests/helpers.py) for the 150+ method test helper library.

**Prerequisites:**
- `cargo install tauri-driver` (WebDriver bridge for Tauri)
- `sudo apt install webkit2gtk-driver` (WebKitWebDriver)
- `sudo apt install xvfb` (headless display)
- Built daemon: `cargo build --release -p pomodoro-daemon`
- Built GUI: `cargo tauri build`

**Test isolation:** Each test file gets a fresh daemon (random port, temp DB), fresh GUI session, and its own Xvfb display. No cross-file contamination.

### Unit Test Coverage
- Auth: seed root, register, login, wrong password, unauthenticated rejection
- Tasks: CRUD, update fields, subtask cascade delete
- Comments: add, list, delete
- Time Reports: add with auto-assign
- Assignees: add, list, remove
- Admin: list users, non-root forbidden
- Rooms: full voting flow, join/leave/kick, role promotion, non-admin forbidden, close, delete, hours-mode accept, auto-advance
- Sprints: CRUD, filtering, task add/remove, detail, start/complete, board grouping, burndown snapshots, duplicate prevention, cascade delete
- Burn Log: log + cancel lifecycle, multi-user summary, cascade on sprint delete
- Task-Sprint mappings endpoint
- Timer state, config, history

## Tech Stack

- **Backend**: Rust, axum 0.8, SQLite (sqlx), bcrypt, jsonwebtoken, utoipa (OpenAPI)
- **Frontend**: Tauri v2, React 19, TypeScript, Tailwind v4, Zustand, Framer Motion, Lucide icons
- **Testing**: tower test utilities, in-memory SQLite, http-body-util
- **i18n**: Zustand-based locale store with English default, extensible to any language
- **Security**: JWT with refresh token rotation, CSRF validation, rate limiting, XOR-encrypted auth storage
