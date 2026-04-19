# E2E Test Suite — pojidora

**1011 tests** across **44 files** covering the Tauri GUI, REST API, all 154 endpoints, WebSocket, security, performance, data integrity, multi-user scenarios, edge cases, and concurrent stress testing.

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

## Writing New Tests

See **[CHEATSHEET.md](CHEATSHEET.md)** for 12 copy-paste test patterns covering every common scenario.

Use the `H()` helper class from **`helpers.py`** — every public method has docstrings and type hints:

```python
from helpers import H

def test_example(self, logged_in):
    h = H()                                    # root user
    task = h.create_task("My Task", estimated=5)
    h.add_comment(task["id"], "Looks good!")
    alice = H.register("alice")                # new user
    code, _ = alice.api_status("PUT", f"/api/tasks/{task['id']}", {"title": "X"})
    assert code == 403                         # alice can't edit root's task
```

Key patterns:
- `H()` — root user helper (lazy auth)
- `H.register("name")` — create user, returns `H` instance
- `h.api_status("METHOD", "/path", body)` — returns `(status_code, response)`
- `h.create_task()`, `h.create_sprint()`, `h.create_room()` — return created object
- `H.assert_task_in_gui(app, "title")` — GUI assertion via WebDriver

All 150+ methods in `helpers.py` are documented. Run `python3 -c "import helpers; help(helpers.H)"` for the full API.

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

## Test Files (44 files, 1011 tests)

### GUI tests

| File | Tests | Coverage |
|------|------:|---------|
| `test_flows.py` | 47 | Login, registration, logout, timer modes, tabs, theme toggle, DOM integrity, multi-user, password validation, session expiry |
| `test_gui_views.py` | 34 | Task detail view, sprint board columns (todo/wip/done), settings persistence after reload, dark/light theme CSS, sidebar navigation |
| `test_gui_interactions.py` | 17 | Keyboard shortcuts (Space/Escape/slash), form submissions (Enter key), dropdowns, theme toggle, sidebar active state, sprint board buttons, comment input |
| `test_regressions.py` | 15 | Stale token auth, password placeholder, React 19 input filling, Xvfb display isolation |
| `test_sprint_lifecycle.py` | 7 | Sprint display, planning, board, start, columns, complete, list |
| `test_labels.py` | 6 | Label CRUD, assign/remove from tasks, GUI verification |
| `test_settings.py` | 5 | Settings display, work duration, estimation mode, persistence |
| `test_dashboard.py` | 5 | History, zero state, task/sprint/room counts |
| `test_room_voting.py` | 5 | Room display, voting status, vote + reveal, member list |

### API exhaustive tests

| File | Tests | Coverage |
|------|------:|---------|
| `test_edge_cases.py` | 68 | Unicode/emoji, long strings (10K chars), empty strings, HTML/SQL injection, null bytes, special chars in usernames, timer boundary values, task priority/estimated boundaries, concurrent login, sprint date edge cases, room vote boundaries |
| `test_status_transitions.py` | 66 | All 56 valid status transitions (parametrized), 8 bulk operations, 2 invalid transitions |
| `test_features.py` | 43 | Recurrence (4 patterns), templates (8), webhooks (12), notifications (3), profile (5), notification prefs (3) |
| `test_error_paths.py` | 41 | Invalid inputs, unauthorized, not-found for tasks/sprints/rooms/epics/labels/teams/auth/admin/comments |
| `test_task_exhaustive.py` | 41 | Every create/update field, all 8 statuses, queries, search, trash, detail, duplicate, reorder |
| `test_endpoints_exhaustive.py` | 39 | Health, auth, admin, profile, timer, session notes, webhooks, CSV import, comments, teams, epics |
| `test_scenarios.py` | 34 | Privilege escalation, cross-user, ownership boundaries, permissions, audit, dependencies |
| `test_config_exhaustive.py` | 32 | Every config field (20), boundary values (7), combinations (5) |
| `test_sprint_exhaustive.py` | 27 | Create fields, update, delete, tasks, roots, burns, analytics, burndown, velocity, compare |
| `test_import_export.py` | 21 | JSON import/export, CSV export, backup/restore, misc endpoints |
| `test_timer_states.py` | 19 | All phases, pause/resume/skip, edge cases, history, multi-skip cycle |
| `test_sprint_transitions.py` | 17 | planning→active→completed, burns, board, scope, roots, carryover, errors |
| `test_room_exhaustive.py` | 15 | Create, detail, lifecycle, multi-user join/leave/remove/role, export, delete |
| `test_coverage_gaps.py` | 11 | Attachment lifecycle (upload/list/download/delete), task sub-resources (burn-total, burn-users, sessions, votes, tasks/full) |
| `test_misc.py` | 11 | Time reporting, watchers, assignees, templates, notifications, password change |
| `test_stress.py` | 10 | Concurrent: 500 task creates, sprint burns, room votes, comments, registrations, 200 rapid GETs |
| `test_advanced.py` | 9 | Export, import JSON, recurrence, webhooks, sprint velocity/burndown/scope |
| `test_task_crud.py` | 8 | CRUD, status, delete/restore, purge, bulk |
| `test_admin.py` | 6 | User list, create, role change, audit, backup |
| `test_epics.py` | 5 | CRUD, add/remove tasks, delete |
| `test_teams.py` | 5 | CRUD, members, delete, settings GUI |
| `test_dependencies.py` | 4 | Add/remove deps, graph API, GUI verify |
| `test_comments.py` | 3 | Add comment, detail API verify, count |

### Security tests

| File | Tests | Coverage |
|------|------:|---------|
| `test_security.py` | 41 | JWT tampering (8), IDOR (10), privilege escalation (7), rate limiter (3), SQL injection (5), path traversal (9) |
| `test_auth_matrix.py` | 46 | 35 auth-required endpoints → 401, 1 public → 200, 7 wrong methods → 405, 3 admin-only → 403 |

### Data integrity & performance

| File | Tests | Coverage |
|------|------:|---------|
| `test_data_integrity.py` | 21 | Task lifecycle counts (create/delete/restore/purge), sprint board column invariant, dependency chain integrity, concurrent burns, import/export round-trip, cross-entity consistency |
| `test_performance.py` | 20 | Startup time (<3s), API latency empty DB (<100ms), latency with 1000 tasks (<500ms), memory (<50MB RSS), concurrent throughput (50 parallel requests <10s), P99 latency |
| `test_idempotency.py` | 23 | Double-delete, double-create, double-start sprint, double-vote, double-burn, double-restore, status toggle |
| `test_input_validation.py` | 34 | Missing required fields (12), wrong types (6), boundary values (16) — empty, whitespace, 10K chars, negative, zero, huge |

### Search & pagination

| File | Tests | Coverage |
|------|------:|---------|
| `test_pagination.py` | 21 | Page/per_page params, x-total-count header, boundary values (0/-1/10000), last page, filters (project/priority/status/combined) |
| `test_search.py` | 24 | FTS5: exact match, case insensitive, partial word, special chars, SQL injection, empty/whitespace, limit params, snippet field, deleted exclusion |

### WebSocket

| File | Tests | Coverage |
|------|------:|---------|
| `test_websocket.py` | 8 | Ticket auth (no ticket/invalid/valid), non-member forbidden, initial state, vote update push, ping keepalive |

### Test infrastructure

| File | Tests | Coverage |
|------|------:|---------|
| `test_helpers.py` | 35 | H() init, token caching, register, api_status, create_task/sprint/room/label/epic/team, logout, list methods, comments, config, raw API |
| `test_api_contracts.py` | 40 | Response schema for task/sprint/room/user/comment/label/team, error format, delete returns 204, create returns 201, list returns array |

### Workflow scenarios

| File | Tests | Coverage |
|------|------:|---------|
| `test_workflow_scenarios.py` | 21 | New developer onboarding, sprint planning meeting, code review day (estimation), end of sprint (burndown + carryover), team lead dashboard, cleanup day, multi-sprint velocity, timer workflow, cross-user collaboration, epic workflow |

## API Reference (for writing new tests)

Key endpoints and their quirks:

- `DELETE` requests must NOT send `Content-Type` with empty body (server returns 400)
- Sprint detail: `GET /api/sprints/{id}` returns `{"sprint": {...}, "tasks": [...]}`
- Task detail: `GET /api/tasks/{id}` returns `{"task": {...}, "comments": [...]}`
- Valid task statuses: `backlog`, `active`, `in_progress`, `blocked`, `completed`, `done`, `estimated`, `archived`
- Valid user roles: `user`, `root` (not "admin")
- Valid room roles: `admin`, `voter`
- Comments field is `content` (not `body`)
- Delete comment: `DELETE /api/comments/{id}` (not nested under tasks)
- Label assign: `PUT /api/tasks/{id}/labels/{label_id}` (PUT, not POST)
- Dependency add: `POST /api/tasks/{id}/dependencies` with `{"depends_on": dep_id}`
- Task list `GET /api/tasks` returns ALL tasks (team visibility)
- Pagination: `?page=1&per_page=10` → `x-total-count`, `x-page`, `x-per-page` headers
- Search: `GET /api/tasks/search?q=term&limit=20` → returns `[{id, title, snippet}]`
- WebSocket: `GET /api/rooms/{id}/ws?ticket=...` — ticket from `POST /api/timer/ticket`
- Attachment upload: `POST /api/tasks/{id}/attachments` with `Content-Type: application/octet-stream` and `X-Filename` header
- Import: `POST /api/import/tasks/json` with `{"tasks": [...]}`
- Sprint compare: `GET /api/sprints/compare?a={id1}&b={id2}`
- Sprint transitions return 405 for invalid state changes (e.g. start already-active)
- After API config change, GUI needs `location.reload()` + re-login
- React 19 inputs require `nativeInputValueSetter` workaround (see `harness.gui_login`)

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

## Performance Benchmarks

`test_performance.py` enforces hard thresholds:

| Metric | Threshold | Actual |
|--------|-----------|--------|
| Cold start | < 3s | ~0.5s |
| Health check | < 100ms | ~1ms |
| List 1000 tasks | < 500ms | ~50ms |
| Create task (loaded) | < 200ms | ~5ms |
| Login (bcrypt) | < 500ms | ~180ms |
| RSS after 1000 tasks | < 50MB | ~15MB |
| 50 parallel requests | < 10s | ~1s |
| P99 latency (100 reqs) | < 500ms | ~5ms |
