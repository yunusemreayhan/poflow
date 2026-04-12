# E2E Test Suite — pomodoroLinux

324 tests across 21 files covering the Tauri GUI, REST API, multi-user scenarios, and concurrent stress testing.

## Quick Start

```bash
cd e2etests
bash run_e2e.sh
```

First run auto-creates a Python venv and installs dependencies. Requires:

- `cargo` (Rust toolchain)
- `tauri-driver` — `cargo install tauri-driver`
- `WebKitWebDriver` — `sudo apt install webkit2gtk-driver`
- `Xvfb` — `sudo apt install xvfb`
- `python3` ≥ 3.10

The daemon binary (`pomodoro-daemon`) is built automatically if missing.

## Architecture

### Per-file isolation

`run_e2e.sh` runs each `test_*.py` as a **separate pytest invocation**. Every file gets:

- A fresh `pomodoro-daemon` process (random port, temp database)
- A fresh Tauri GUI session via `TauriWebDriver`
- Complete state isolation — no cross-file contamination

### Headless display

Xvfb starts automatically on a random display (`:99`–`:598`). Multiple suite runs can coexist on the same machine.

### Daemon lifecycle

The `harness.Daemon` class (in `harness.py`) manages the daemon:

- Picks a random free port
- Creates a temp directory for the database
- Sets `POMODORO_NO_RATE_LIMIT=1` to disable auth and API rate limiters
- Registers a root user on startup
- Cleans up temp files on stop

Key constants in `harness.py`:

| Constant | Value | Purpose |
|----------|-------|---------|
| `ROOT_PASSWORD` | `TestRoot1` | Default root credentials |
| `JWT_SECRET` | `test-secret-...` | Fixed JWT secret for test predictability |
| `BASE_URL` | `http://127.0.0.1:{port}` | Set dynamically after daemon starts |

### GUI automation

Tests use `desktop-pilot` (`tauriTester/` submodule) which drives the Tauri app through the WebDriver protocol — direct DOM access, no OCR or screenshots.

### API helper pattern

Most test files define a local `api()` function:

```python
def api(method, path, body=None, token=None):
    # Sends JSON requests to harness.BASE_URL
    # Returns parsed JSON response
```

Multi-user tests use a `tok()` helper to get auth tokens:

```python
def tok(user="root", pw=ROOT_PASSWORD):
    return api("POST", "/api/auth/login", {"username": user, "password": pw})["token"]
```

## Running specific tests

```bash
# Single file
bash run_e2e.sh test_flows.py

# Single test
bash run_e2e.sh test_flows.py::TestLogin::test_login_shows_timer

# With pytest flags
bash run_e2e.sh test_stress.py -v --tb=long

# Just the API tests (no GUI needed, fastest)
bash run_e2e.sh test_stress.py test_config_exhaustive.py test_sprint_exhaustive.py -v
```

## Test Files

### GUI + API (use `logged_in` fixture)

| File | Tests | What it covers |
|------|------:|----------------|
| `test_flows.py` | 47 | Login, registration, logout, timer modes, tabs, theme toggle, DOM integrity, multi-user GUI, password validation, session expiry |
| `test_settings.py` | 5 | Settings display, work duration, estimation mode, persistence across reload |
| `test_dashboard.py` | 5 | History, zero state, task/sprint/room counts |
| `test_sprint_lifecycle.py` | 7 | Sprint display, planning, board, start, columns, complete, list |
| `test_labels.py` | 6 | Label CRUD, assign/remove from tasks, GUI verification |
| `test_room_voting.py` | 5 | Room display, voting status, vote + reveal, member list |

### API-only (use `logged_in` but only for daemon startup)

| File | Tests | What it covers |
|------|------:|----------------|
| `test_task_exhaustive.py` | 41 | Every create/update field, all 8 statuses, queries, search, trash, detail, duplicate, reorder, errors |
| `test_endpoints_exhaustive.py` | 39 | Health, auth refresh/logout, admin ops, profile, timer lifecycle, session notes, webhooks CRUD, CSV import, comment edit/delete, team roots/members, epic snapshots |
| `test_scenarios.py` | 34 | Privilege escalation, cross-user assignment, sprint burn multi-user, ownership boundaries, comment permissions, full team workflow, audit trail, dependency permissions, watcher notifications |
| `test_config_exhaustive.py` | 32 | Every config field (20), boundary values (7), combinations (5) |
| `test_sprint_exhaustive.py` | 27 | Create fields, update, delete, tasks, roots, burns, analytics, burndown, velocity, compare, carryover |
| `test_room_exhaustive.py` | 15 | Create, detail, lifecycle, multi-user join/leave/remove/role, export, delete |
| `test_misc.py` | 11 | Time reporting, watchers, assignees, templates, notifications, password change |
| `test_stress.py` | 10 | Concurrent task creation (500), sprint burns, room voting, comments, status updates, duplicate registration, watch/unwatch, sprint task adds, high load (200 rapid requests) |
| `test_advanced.py` | 9 | Export, import JSON, recurrence, webhooks, sprint velocity/burndown/scope |
| `test_task_crud.py` | 8 | CRUD, status transitions, delete/restore, purge, bulk operations |
| `test_admin.py` | 6 | User list, create, role change, audit log, backup |
| `test_epics.py` | 5 | CRUD, add/remove tasks, delete |
| `test_teams.py` | 5 | CRUD, members, delete, settings GUI |
| `test_dependencies.py` | 4 | Add/remove deps, graph API, GUI verification |
| `test_comments.py` | 3 | Add comment, detail API verify, count |

## API Reference (for writing new tests)

Key endpoints and their quirks:

- `DELETE` requests must NOT send `Content-Type` with empty body (server returns 400)
- Sprint detail: `GET /api/sprints/{id}` returns `{"sprint": {...}, "tasks": [...]}`
- Valid task statuses: `backlog`, `active`, `in_progress`, `blocked`, `completed`, `done`, `estimated`, `archived`
- Valid user roles: `user`, `root` (not "admin")
- Valid room roles: `admin`, `voter`
- Comments field is `content` (not `body`)
- Task list `GET /api/tasks` returns ALL tasks (team visibility)
- Epic tasks: `POST /api/epics/{id}/tasks` with `{"task_ids": [...]}`
- Reorder: `POST /api/tasks/reorder` with `{"orders": [[task_id, sort_order], ...]}`
- Import: `POST /api/import/tasks/json` with `{"tasks": [...]}`
- Sprint compare: `GET /api/sprints/compare?a={id1}&b={id2}`
- After API config change, GUI needs `location.reload()` + re-login

## Stress Testing

`test_stress.py` uses `concurrent.futures.ThreadPoolExecutor` to hammer the daemon:

- 10 users × 50 tasks = 500 concurrent creates
- 5 users × 10 tasks = 50 concurrent sprint burns
- 8 concurrent room votes
- 5 users × 10 = 50 concurrent comments
- 10 concurrent status updates on same task
- 10 concurrent duplicate registrations (exactly 1 succeeds)
- 200 rapid GET requests with 20 threads

All pass — SQLite WAL mode + Rust handles concurrent access correctly.
