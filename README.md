# Pomodoro Linux

A full-featured multi-user Pomodoro timer for Linux with a Rust HTTP backend, Tauri v2 GUI, hierarchical task management, sprint planning, estimation rooms, and time tracking. Packaged as a single `.deb`.

## Features

### Timer
- Pomodoro work/break cycles with configurable durations
- Auto-start breaks and work sessions
- Desktop notifications on session completion
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

### Sprint Management
- Create sprints with name, project, goal, and date range
- Sprint lifecycle: planning → active → completed
- **Board tab**: Kanban columns (Todo / In Progress / Done) with click-to-change-status
- **Backlog tab**: Add/remove tasks using the full hierarchical task tree
- **Burndown tab**: SVG line chart with ideal vs actual remaining (toggle points/hours/tasks)
- **Summary tab**: Stats cards, velocity, per-user progress bars
- Auto-snapshot: hourly background task captures burndown data for active sprints
- Sprint badges on all task views (green = active sprint, pale green = past sprint)

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
- Quick-accept, custom value accept, re-vote
- Auto-advance to next unestimated task after accept
- Admin inline edit task title/description from voting screen
- Room roles: admin/voter with promote/demote/kick
- Vote history with per-user breakdown

### Multi-User
- JWT authentication (bcrypt + 7-day tokens)
- First registered user becomes root (seed: root/root)
- Root users can manage all users and override ownership
- Everyone sees all data; ownership controls edit/delete
- Profile management (change username/password)
- Admin panel for user role management

### Architecture
- **Backend**: Rust + axum HTTP server on port 9090
- **Frontend**: Tauri v2 + React + TypeScript + Tailwind v4
- **Database**: SQLite with foreign key constraints
- **Auth**: JWT with user_id-based identity (usernames are changeable)
- **API**: OpenAPI/Swagger UI at `/swagger-ui/`
- **State**: Zustand store with Tauri invoke → reqwest bridge

## Database Schema (12 tables)

All user references use `user_id INTEGER REFERENCES users(id)` — usernames are resolved via JOINs. This means usernames can be changed without breaking any foreign key relationships.

| Table | Purpose |
|---|---|
| `users` | id, username (unique, changeable), password_hash, role, created_at |
| `tasks` | Hierarchical tasks with user_id FK, parent_id self-ref, status, estimates |
| `sessions` | Pomodoro timer sessions with user_id FK |
| `comments` | Comments on tasks with user_id FK |
| `time_reports` | Hour tracking per task with user_id FK, auto-assigns user |
| `task_assignees` | Many-to-many task↔user with user_id FK |
| `rooms` | Estimation rooms with creator_id FK |
| `room_members` | Room membership with user_id FK and role |
| `room_votes` | Votes with user_id FK, unique per room+task+user |
| `sprints` | Sprint metadata with created_by_id FK |
| `sprint_tasks` | Sprint↔task mapping with added_by_id FK |
| `sprint_daily_stats` | Burndown snapshots per sprint per day |
| `burn_log` | Unified burn tracking: manual entries, timer completions, time reports. Has source (manual/timer/time_report), optional sprint_id, optional session_id, soft-delete via cancelled_by_id |

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
| GET | `/api/history` | Session history (filter: ?from=&to=) |
| GET | `/api/stats` | Day stats (filter: ?days=) |

## Installation

```bash
# Build
cd gui && cargo tauri build --bundles deb

# Install
sudo dpkg -i target/release/bundle/deb/Pomodoro_0.1.0_amd64.deb

# The daemon auto-starts via systemd user service
systemctl --user status pomodoro.service

# Access Swagger UI
open http://localhost:9090/swagger-ui/
```

## Testing

40 integration tests run automatically before every build (configured in `tauri.conf.json`):

```bash
cargo test -p pomodoro-daemon
```

Tests use in-memory SQLite — no disk I/O, fully isolated, no port conflicts.

### Test Coverage
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
