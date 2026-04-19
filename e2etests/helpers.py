"""High-level test helpers for pojidora E2E tests.

Import these to write tests in 3-5 lines instead of raw API calls.
Every helper handles auth, returns the created object, and is composable.

Usage:
    from helpers import H
    h = H()                          # uses root token
    task = h.create_task("My Task")
    h.add_comment(task["id"], "hello")
    h.assert_task_exists(task["id"])
"""

import json, os, urllib.request, urllib.error
import harness
from harness import ROOT_PASSWORD

_ID = os.getpid()


def _api(method, path, body=None, token=None):
    """Raw API call — returns parsed JSON."""
    url = harness.BASE_URL
    if body is not None:
        data = json.dumps(body).encode()
        hdrs = {"Content-Type": "application/json", "X-Requested-With": "test"}
    elif method in ("POST", "PUT"):
        data, hdrs = b"", {"Content-Type": "application/json", "X-Requested-With": "test"}
    else:
        data, hdrs = None, {"X-Requested-With": "test"}
    if token:
        hdrs["Authorization"] = f"Bearer {token}"
    resp = urllib.request.urlopen(
        urllib.request.Request(f"{url}{path}", data=data, headers=hdrs, method=method), timeout=10)
    raw = resp.read().decode()
    return json.loads(raw) if raw else {}


def _api_status(method, path, body=None, token=None):
    """Raw API call — returns (status_code, response_or_error)."""
    url = harness.BASE_URL
    if body is not None:
        data = json.dumps(body).encode()
        hdrs = {"Content-Type": "application/json", "X-Requested-With": "test"}
    elif method in ("POST", "PUT"):
        data, hdrs = b"", {"Content-Type": "application/json", "X-Requested-With": "test"}
    else:
        data, hdrs = None, {"X-Requested-With": "test"}
    if token:
        hdrs["Authorization"] = f"Bearer {token}"
    try:
        resp = urllib.request.urlopen(
            urllib.request.Request(f"{url}{path}", data=data, headers=hdrs, method=method), timeout=10)
        raw = resp.read().decode()
        return resp.status, json.loads(raw) if raw else {}
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()[:500]


class H:
    """High-level test helper. One instance per user."""

    def __init__(self, user: str = "root", password: str = ROOT_PASSWORD) -> None:
        """Create a helper bound to a specific user.

        Args:
            user: Username to authenticate as.
            password: Password for authentication.
        """
        self.user = user
        self.password = password
        self._token = None

    @property
    def token(self) -> str:
        """JWT token, lazily fetched on first access."""
        if not self._token:
            self._token = _api("POST", "/api/auth/login",
                               {"username": self.user, "password": self.password})["token"]
        return self._token

    def api(self, method: str, path: str, body: dict | None = None) -> dict:
        """Make an authenticated API call. Raises on HTTP error."""
        return _api(method, path, body, self.token)

    def api_status(self, method: str, path: str, body: dict | None = None) -> tuple[int, dict | str]:
        """Make an authenticated API call. Returns (status_code, response)."""
        return _api_status(method, path, body, self.token)

    # ── Auth ────────────────────────────────────────────────────

    @staticmethod
    def register(username: str, password: str = "TestPass1") -> "H":
        """Register a new user and return an H instance for them. Idempotent."""
        try:
            _api("POST", "/api/auth/register", {"username": username, "password": password})
        except Exception:
            pass
        return H(username, password)

    def logout(self) -> None:
        """Log out, invalidating the current token."""
        self.api("POST", "/api/auth/logout")
        self._token = None

    def refresh(self) -> dict:
        """Refresh the JWT token."""
        r = self.api("POST", "/api/auth/refresh")
        self._token = r.get("token", self._token)
        return r

    # ── Tasks ───────────────────────────────────────────────────

    def create_task(self, title: str | None = None, project: str | None = None, **kwargs) -> dict:
        """Create a task. Returns the created task dict with ``id``, ``title``, etc."""
        body = {"title": title or f"Task_{_ID}_{id(self)}", "project": project or "Test"}
        body.update(kwargs)
        return self.api("POST", "/api/tasks", body)

    def get_task(self, task_id: int) -> dict:
        """Get task detail. Returns ``{"task": {...}, "comments": [...]}``."""
        return self.api("GET", f"/api/tasks/{task_id}")

    def update_task(self, task_id: int, **kwargs) -> dict:
        """Update task fields. Pass any field as keyword arg."""
        return self.api("PUT", f"/api/tasks/{task_id}", kwargs)

    def delete_task(self, task_id: int) -> tuple[int, dict | str]:
        """Soft-delete a task. Returns (status, response)."""
        return _api_status("DELETE", f"/api/tasks/{task_id}", token=self.token)

    def restore_task(self, task_id: int) -> dict:
        """Restore a soft-deleted task."""
        return self.api("POST", f"/api/tasks/{task_id}/restore")

    def purge_task(self, task_id: int) -> tuple[int, dict | str]:
        """Permanently delete a task. Returns (status, response)."""
        return _api_status("DELETE", f"/api/tasks/{task_id}/permanent", token=self.token)

    def list_tasks(self) -> list[dict]:
        """List all tasks (team-visible)."""
        return self.api("GET", "/api/tasks")

    def list_trash(self) -> list[dict]:
        """List soft-deleted tasks."""
        return self.api("GET", "/api/tasks/trash")

    def search_tasks(self, q: str) -> list[dict]:
        """Search tasks by query string."""
        return self.api("GET", f"/api/tasks/search?q={q}")

    def duplicate_task(self, task_id: int) -> dict:
        """Duplicate a task. Returns the new task."""
        return self.api("POST", f"/api/tasks/{task_id}/duplicate")

    def reorder_tasks(self, orders: list[list[int]]) -> dict:
        """Reorder tasks. ``orders`` is ``[[task_id, sort_order], ...]``."""
        return self.api("POST", "/api/tasks/reorder", {"orders": orders})

    def bulk_status(self, task_ids: list[int], status: str) -> dict:
        """Set status for multiple tasks at once."""
        return self.api("PUT", "/api/tasks/bulk-status", {"task_ids": task_ids, "status": status})

    def set_task_status(self, task_id: int, status: str) -> dict:
        """Shorthand to change a task's status."""
        return self.update_task(task_id, status=status)

    # ── Comments ────────────────────────────────────────────────

    def add_comment(self, task_id: int, content: str = "Test comment") -> dict:
        """Add a comment to a task. Returns the comment dict."""
        return self.api("POST", f"/api/tasks/{task_id}/comments", {"content": content})

    def list_comments(self, task_id: int) -> list[dict]:
        """List all comments on a task."""
        return self.api("GET", f"/api/tasks/{task_id}/comments")

    def edit_comment(self, comment_id: int, content: str) -> dict:
        """Edit a comment's content."""
        return self.api("PUT", f"/api/comments/{comment_id}", {"content": content})

    def delete_comment(self, comment_id: int) -> tuple[int, dict | str]:
        """Delete a comment. Returns (status, response)."""
        return _api_status("DELETE", f"/api/comments/{comment_id}", token=self.token)

    # ── Labels ──────────────────────────────────────────────────

    def create_label(self, name: str | None = None, color: str = "#ff0000") -> dict:
        """Create a label. Returns the label dict."""
        return self.api("POST", "/api/labels", {"name": name or f"Lbl_{_ID}", "color": color})

    def list_labels(self) -> list[dict]:
        """List all labels."""
        return self.api("GET", "/api/labels")

    def delete_label(self, label_id):
        """Delete a label by ID."""
        return _api_status("DELETE", f"/api/labels/{label_id}", token=self.token)

    def assign_label(self, task_id, label_id):
        """Assign a label to a task."""
        return self.api("PUT", f"/api/tasks/{task_id}/labels/{label_id}")

    def remove_label(self, task_id, label_id):
        """Remove a label from a task."""
        return _api_status("DELETE", f"/api/tasks/{task_id}/labels/{label_id}", token=self.token)

    def task_labels(self, task_id):
        """List labels assigned to a task."""
        return self.api("GET", f"/api/tasks/{task_id}/labels")

    # ── Dependencies ────────────────────────────────────────────

    def add_dependency(self, task_id, dep_id):
        """Add a dependency between two tasks."""
        return self.api("POST", f"/api/tasks/{task_id}/dependencies", {"dependency_id": dep_id})

    def remove_dependency(self, task_id, dep_id):
        """Remove a dependency link."""
        return _api_status("DELETE", f"/api/tasks/{task_id}/dependencies/{dep_id}", token=self.token)

    def task_dependencies(self, task_id):
        """List dependencies of a task."""
        return self.api("GET", f"/api/tasks/{task_id}/dependencies")

    def all_dependencies(self):
        """List all dependency links."""
        return self.api("GET", "/api/dependencies")

    # ── Assignees ───────────────────────────────────────────────

    def assign_user(self, task_id, username):
        """Assign a user to a task."""
        return self.api("POST", f"/api/tasks/{task_id}/assignees", {"username": username})

    def remove_assignee(self, task_id, username):
        """Remove an assignee from a task."""
        return _api_status("DELETE", f"/api/tasks/{task_id}/assignees/{username}", token=self.token)

    def task_assignees(self, task_id):
        """List assignees of a task."""
        return self.api("GET", f"/api/tasks/{task_id}/assignees")

    # ── Watchers ────────────────────────────────────────────────

    def watch_task(self, task_id):
        """Watch a task for notifications."""
        return self.api("POST", f"/api/tasks/{task_id}/watch")

    def unwatch_task(self, task_id):
        """Stop watching a task."""
        return _api_status("DELETE", f"/api/tasks/{task_id}/watch", token=self.token)

    def task_watchers(self, task_id):
        """List watchers of a task."""
        return self.api("GET", f"/api/tasks/{task_id}/watchers")

    def watched_tasks(self):
        """List all tasks the current user watches."""
        return self.api("GET", "/api/watched")

    # ── Time logging ────────────────────────────────────────────

    def log_time(self, task_id, hours=1.0, note=""):
        """Log time spent on a task."""
        return self.api("POST", f"/api/tasks/{task_id}/time", {"hours": hours, "note": note})

    def task_time(self, task_id):
        """List time entries for a task."""
        return self.api("GET", f"/api/tasks/{task_id}/time")

    def task_time_summary(self, task_id):
        """Get aggregated time summary for a task."""
        return self.api("GET", f"/api/tasks/{task_id}/time-summary")

    # ── Recurrence ──────────────────────────────────────────────

    def set_recurrence(self, task_id, pattern="daily", next_due="2026-06-01"):
        """Set a recurrence pattern: ``daily``, ``weekly``, ``biweekly``, ``monthly``."""
        return self.api("PUT", f"/api/tasks/{task_id}/recurrence",
                         {"pattern": pattern, "next_due": next_due})

    def get_recurrence(self, task_id):
        """Get the recurrence rule for a task."""
        return self.api("GET", f"/api/tasks/{task_id}/recurrence")

    def remove_recurrence(self, task_id):
        """Remove recurrence from a task."""
        return _api_status("DELETE", f"/api/tasks/{task_id}/recurrence", token=self.token)

    # ── Sprints ─────────────────────────────────────────────────

    def create_sprint(self, name: str | None = None, start: str = "2026-05-01", end: str = "2026-05-15", **kwargs) -> dict:
        """Create a sprint. Returns the sprint dict with ``id``, ``name``, ``status``."""
        body = {"name": name or f"Sprint_{_ID}", "start_date": start, "end_date": end}
        body.update(kwargs)
        return self.api("POST", "/api/sprints", body)

    def get_sprint(self, sprint_id):
        """Get sprint detail (``{"sprint": {...}, "tasks": [...]}``}."""
        return self.api("GET", f"/api/sprints/{sprint_id}")

    def update_sprint(self, sprint_id, **kwargs):
        """Update sprint fields."""
        return self.api("PUT", f"/api/sprints/{sprint_id}", kwargs)

    def delete_sprint(self, sprint_id):
        """Delete a sprint."""
        return _api_status("DELETE", f"/api/sprints/{sprint_id}", token=self.token)

    def list_sprints(self):
        """List all sprints."""
        return self.api("GET", "/api/sprints")

    def start_sprint(self, sprint_id: int) -> dict:
        """Transition sprint from planning → active."""
        return self.api("POST", f"/api/sprints/{sprint_id}/start")

    def complete_sprint(self, sprint_id: int) -> dict:
        """Transition sprint from active → completed."""
        return self.api("POST", f"/api/sprints/{sprint_id}/complete")

    def add_sprint_tasks(self, sprint_id: int, task_ids: list[int]) -> dict:
        """Add tasks to a sprint."""
        return self.api("POST", f"/api/sprints/{sprint_id}/tasks", {"task_ids": task_ids})

    def remove_sprint_task(self, sprint_id, task_id):
        """Remove a task from a sprint."""
        return _api_status("DELETE", f"/api/sprints/{sprint_id}/tasks/{task_id}", token=self.token)

    def sprint_tasks(self, sprint_id):
        """List tasks in a sprint."""
        return self.api("GET", f"/api/sprints/{sprint_id}/tasks")

    def sprint_board(self, sprint_id):
        """Get the sprint board (columns by status)."""
        return self.api("GET", f"/api/sprints/{sprint_id}/board")

    def burn(self, sprint_id: int, task_id: int, points: float = 1.0, hours: float = 0.5) -> dict:
        """Log a burn entry on an active sprint."""
        return self.api("POST", f"/api/sprints/{sprint_id}/burn",
                         {"task_id": task_id, "points": points, "hours": hours})

    def cancel_burn(self, sprint_id, burn_id):
        """Cancel a burn entry."""
        return self.api("DELETE", f"/api/sprints/{sprint_id}/burns/{burn_id}")

    def sprint_burns(self, sprint_id):
        """List all burn entries for a sprint."""
        return self.api("GET", f"/api/sprints/{sprint_id}/burns")

    def sprint_burndown(self, sprint_id):
        """Get burndown chart data for a sprint."""
        return self.api("GET", f"/api/sprints/{sprint_id}/burndown")

    def sprint_burn_summary(self, sprint_id):
        """Get burn summary grouped by task."""
        return self.api("GET", f"/api/sprints/{sprint_id}/burn-summary")

    def sprint_snapshot(self, sprint_id):
        """Take a daily snapshot of sprint progress."""
        return self.api("POST", f"/api/sprints/{sprint_id}/snapshot")

    def sprint_carryover(self, sprint_id: int) -> dict:
        """Create a new sprint from incomplete tasks of a completed sprint."""
        return self.api("POST", f"/api/sprints/{sprint_id}/carryover")

    def sprint_roots(self, sprint_id):
        """List root task IDs for a sprint."""
        return self.api("GET", f"/api/sprints/{sprint_id}/roots")

    def add_sprint_root(self, sprint_id, task_id):
        """Mark a task as a root task in a sprint."""
        return self.api("POST", f"/api/sprints/{sprint_id}/roots", {"task_ids": [task_id]})

    def remove_sprint_root(self, sprint_id, task_id):
        """Remove a root task designation."""
        return _api_status("DELETE", f"/api/sprints/{sprint_id}/roots/{task_id}", token=self.token)

    def sprint_scope(self, sprint_id):
        """Get scope change history for a sprint."""
        return self.api("GET", f"/api/sprints/{sprint_id}/scope")

    def sprint_compare(self, id_a, id_b):
        """Compare two sprints side by side."""
        return self.api("GET", f"/api/sprints/compare?a={id_a}&b={id_b}")

    def velocity(self):
        """Get velocity data across sprints."""
        return self.api("GET", "/api/sprints/velocity")

    def global_burndown(self):
        """Get global burndown across all sprints."""
        return self.api("GET", "/api/sprints/burndown")

    # ── Rooms ───────────────────────────────────────────────────

    def create_room(self, name: str | None = None, unit: str = "points") -> dict:
        """Create an estimation room. Returns the room dict."""
        return self.api("POST", "/api/rooms", {"name": name or f"Room_{_ID}", "estimation_unit": unit})

    def get_room(self, room_id):
        """Get room detail including votes and members."""
        return self.api("GET", f"/api/rooms/{room_id}")

    def list_rooms(self):
        """List all estimation rooms."""
        return self.api("GET", "/api/rooms")

    def delete_room(self, room_id):
        """Delete an estimation room."""
        return _api_status("DELETE", f"/api/rooms/{room_id}", token=self.token)

    def join_room(self, room_id):
        """Join an estimation room."""
        return self.api("POST", f"/api/rooms/{room_id}/join")

    def leave_room(self, room_id):
        """Leave an estimation room."""
        return self.api("POST", f"/api/rooms/{room_id}/leave")

    def start_voting(self, room_id: int, task_id: int) -> dict:
        """Start a voting round for a task in a room."""
        return self.api("POST", f"/api/rooms/{room_id}/start-voting", {"task_id": task_id})

    def vote(self, room_id: int, value: float) -> dict:
        """Cast a vote in a room."""
        return self.api("POST", f"/api/rooms/{room_id}/vote", {"value": value})

    def reveal_votes(self, room_id: int) -> dict:
        """Reveal all votes in a room."""
        return self.api("POST", f"/api/rooms/{room_id}/reveal")

    def accept_estimate(self, room_id: int, value: float) -> dict:
        """Accept the estimate and update the task."""
        return self.api("POST", f"/api/rooms/{room_id}/accept", {"value": value})

    def close_room(self, room_id):
        """Close a room (end all voting)."""
        return self.api("POST", f"/api/rooms/{room_id}/close")

    def remove_room_member(self, room_id, username):
        """Remove a member from a room."""
        return _api_status("DELETE", f"/api/rooms/{room_id}/members/{username}", token=self.token)

    def set_room_role(self, room_id, username, role):
        """Set a member's role in a room (``admin`` or ``voter``)."""
        return self.api("PUT", f"/api/rooms/{room_id}/role", {"username": username, "role": role})

    def export_room(self, room_id):
        """Export room voting history."""
        return self.api("GET", f"/api/rooms/{room_id}/export")

    # ── Epics ───────────────────────────────────────────────────

    def create_epic(self, name: str | None = None) -> dict:
        """Create an epic group. Returns the epic dict."""
        return self.api("POST", "/api/epics", {"name": name or f"Epic_{_ID}"})

    def get_epic(self, epic_id):
        """Get epic detail with task IDs."""
        return self.api("GET", f"/api/epics/{epic_id}")

    def list_epics(self):
        """List all epics."""
        return self.api("GET", "/api/epics")

    def delete_epic(self, epic_id):
        """Delete an epic."""
        return _api_status("DELETE", f"/api/epics/{epic_id}", token=self.token)

    def add_epic_tasks(self, epic_id: int, task_ids: list[int]) -> dict:
        """Add tasks to an epic."""
        return self.api("POST", f"/api/epics/{epic_id}/tasks", {"task_ids": task_ids})

    def remove_epic_task(self, epic_id, task_id):
        """Remove a task from an epic."""
        return _api_status("DELETE", f"/api/epics/{epic_id}/tasks/{task_id}", token=self.token)

    def epic_snapshot(self, epic_id):
        """Take a snapshot of epic progress."""
        return self.api("POST", f"/api/epics/{epic_id}/snapshot")

    # ── Teams ───────────────────────────────────────────────────

    def create_team(self, name: str | None = None) -> dict:
        """Create a team. Returns the team dict."""
        return self.api("POST", "/api/teams", {"name": name or f"Team_{_ID}"})

    def get_team(self, team_id):
        """Get team detail with members."""
        return self.api("GET", f"/api/teams/{team_id}")

    def list_teams(self):
        """List all teams."""
        return self.api("GET", "/api/teams")

    def delete_team(self, team_id):
        """Delete a team."""
        return _api_status("DELETE", f"/api/teams/{team_id}", token=self.token)

    def add_team_member(self, team_id, user_id):
        """Add a user to a team by user ID."""
        return self.api("POST", f"/api/teams/{team_id}/members", {"user_id": user_id})

    def remove_team_member(self, team_id, user_id):
        """Remove a user from a team."""
        return _api_status("DELETE", f"/api/teams/{team_id}/members/{user_id}", token=self.token)

    def my_teams(self):
        """List teams the current user belongs to."""
        return self.api("GET", "/api/me/teams")

    def team_scope(self, team_id):
        """Get scope (task IDs) for a team."""
        return self.api("GET", f"/api/teams/{team_id}/scope")

    def add_team_root(self, team_id, task_id):
        """Add a root task to a team."""
        return self.api("POST", f"/api/teams/{team_id}/roots", {"task_id": task_id})

    def remove_team_root(self, team_id, task_id):
        """Remove a root task from a team."""
        return _api_status("DELETE", f"/api/teams/{team_id}/roots/{task_id}", token=self.token)

    # ── Templates ───────────────────────────────────────────────

    def create_template(self, name: str | None = None, data: dict | None = None) -> dict:
        """Create a task template. Use ``{{today}}`` and ``{{username}}`` as variables."""
        return self.api("POST", "/api/templates", {
            "name": name or f"Tmpl_{_ID}",
            "data": data or {"title": "{{today}} standup", "project": "Daily"}
        })

    def list_templates(self):
        """List all task templates."""
        return self.api("GET", "/api/templates")

    def delete_template(self, template_id):
        """Delete a task template."""
        return _api_status("DELETE", f"/api/templates/{template_id}", token=self.token)

    def instantiate_template(self, template_id: int) -> dict:
        """Create a task from a template, resolving variables."""
        return self.api("POST", f"/api/templates/{template_id}/instantiate")

    # ── Webhooks ────────────────────────────────────────────────

    def create_webhook(self, url: str = "https://example.com/hook", events: str = "*", secret: str | None = None) -> dict:
        """Create a webhook. Events: ``task.created``, ``task.updated``, etc. or ``*``."""
        body = {"url": url, "events": events}
        if secret:
            body["secret"] = secret
        return self.api("POST", "/api/webhooks", body)

    def list_webhooks(self):
        """List all webhooks."""
        return self.api("GET", "/api/webhooks")

    def delete_webhook(self, webhook_id):
        """Delete a webhook."""
        return _api_status("DELETE", f"/api/webhooks/{webhook_id}", token=self.token)

    # ── Timer ───────────────────────────────────────────────────

    def start_timer(self, task_id: int | None = None) -> dict:
        """Start a focus timer, optionally linked to a task."""
        return self.api("POST", "/api/timer/start", {"task_id": task_id})

    def stop_timer(self) -> dict:
        """Stop the timer and record the session."""
        return self.api("POST", "/api/timer/stop")

    def pause_timer(self) -> dict:
        """Pause the running timer."""
        return self.api("POST", "/api/timer/pause")

    def resume_timer(self) -> dict:
        """Resume a paused timer."""
        return self.api("POST", "/api/timer/resume")

    def skip_timer(self) -> dict:
        """Skip to the next phase (work → break or break → work)."""
        return self.api("POST", "/api/timer/skip")

    def timer_state(self) -> dict:
        """Get current timer state: ``phase``, ``status``, ``elapsed_s``, ``duration_s``."""
        return self.api("GET", "/api/timer")

    def timer_active(self):
        """Get the currently active timer session, if any."""
        return self.api("GET", "/api/timer/active")

    def timer_ticket(self):
        """Get a timer ticket (for WebSocket auth)."""
        return self.api("POST", "/api/timer/ticket")

    # ── Config ──────────────────────────────────────────────────

    def get_config(self) -> dict:
        """Get the full config dict."""
        return self.api("GET", "/api/config")

    def update_config(self, **kwargs) -> dict:
        """Update config fields. Merges with current config."""
        cfg = self.get_config()
        cfg.update(kwargs)
        return self.api("PUT", "/api/config", cfg)

    # ── History / Stats / Export ─────────────────────────────────

    def history(self) -> list[dict]:
        """List completed pomodoro sessions."""
        return self.api("GET", "/api/history")

    def stats(self):
        """Get daily statistics."""
        return self.api("GET", "/api/stats")

    def export_sessions(self, fmt="json"):
        """Export pomodoro sessions. ``fmt``: ``json`` or ``csv``."""
        return self.api("GET", f"/api/export/sessions?format={fmt}")

    def export_tasks(self):
        """Export all tasks as JSON."""
        return self.api("GET", "/api/export/tasks")

    def export_burns(self, sprint_id):
        """Export burn entries for a sprint."""
        return self.api("GET", f"/api/export/burns/{sprint_id}")

    def import_tasks_json(self, tasks: list[dict]) -> dict:
        """Import tasks from a JSON array."""
        return self.api("POST", "/api/import/tasks/json", {"tasks": tasks})

    # ── Admin ───────────────────────────────────────────────────

    def admin_users(self) -> list[dict]:
        """List all users (root only)."""
        return self.api("GET", "/api/admin/users")

    def admin_set_role(self, user_id: int, role: str) -> dict:
        """Change a user's role. Valid roles: ``user``, ``root``."""
        return self.api("PUT", f"/api/admin/users/{user_id}/role", {"role": role})

    def admin_reset_password(self, user_id, password):
        """Reset a user's password (root only)."""
        return self.api("PUT", f"/api/admin/users/{user_id}/password", {"password": password})

    def admin_delete_user(self, user_id):
        """Delete a user (root only)."""
        return _api_status("DELETE", f"/api/admin/users/{user_id}", token=self.token)

    def admin_backup(self):
        """Create a database backup (root only)."""
        return self.api("POST", "/api/admin/backup")

    def admin_backups(self):
        """List available backups (root only)."""
        return self.api("GET", "/api/admin/backups")

    def admin_restore(self, backup_name):
        """Restore from a named backup (root only)."""
        return self.api("POST", "/api/admin/restore", {"name": backup_name})

    # ── Profile ─────────────────────────────────────────────────

    def update_profile(self, **kwargs) -> dict:
        """Update profile. Pass ``username``, ``password``, ``current_password``."""
        return self.api("PUT", "/api/profile", kwargs)

    def get_notif_prefs(self):
        """Get notification preferences (6 event types)."""
        return self.api("GET", "/api/profile/notifications")

    def update_notif_prefs(self, prefs):
        """Update notification preferences."""
        return self.api("PUT", "/api/profile/notifications", prefs)

    # ── Notifications ───────────────────────────────────────────

    def notifications(self):
        """List all notifications."""
        return self.api("GET", "/api/notifications")

    def unread_count(self):
        """Get unread notification count."""
        return self.api("GET", "/api/notifications/unread")

    def mark_read(self):
        """Mark all notifications as read."""
        return self.api("POST", "/api/notifications/read")

    # ── Misc ────────────────────────────────────────────────────

    def health(self):
        """Health check (no auth required)."""
        return _api("GET", "/api/health")

    def audit(self):
        """Get audit log entries."""
        return self.api("GET", "/api/audit")

    def users(self):
        """List all users (username + ID)."""
        return self.api("GET", "/api/users")

    def assignees_list(self):
        """List all task assignees."""
        return self.api("GET", "/api/assignees")

    def burn_totals(self):
        """Get burn totals across all sprints."""
        return self.api("GET", "/api/burn-totals")

    def user_hours(self):
        """Get hours report grouped by user."""
        return self.api("GET", "/api/reports/user-hours")

    def task_sprints(self):
        """Get task-to-sprint mapping."""
        return self.api("GET", "/api/task-sprints")

    def update_session_note(self, session_id, note):
        """Update the note on a completed pomodoro session."""
        return self.api("PUT", f"/api/sessions/{session_id}/note", {"note": note})

    # ── GUI assertions ──────────────────────────────────────────

    @staticmethod
    def assert_task_in_gui(app, title: str) -> None:
        """Navigate to Tasks tab and assert the title appears in the page."""
        from harness import click_tab
        click_tab(app, "Timer")
        click_tab(app, "Tasks")
        assert title in app.page_source(), f"Task '{title}' not found in GUI"

    @staticmethod
    def assert_sprint_status(app, name: str, status: str) -> None:
        """Navigate to Sprints tab and assert sprint name + status appear."""
        from harness import click_tab
        click_tab(app, "Timer")
        click_tab(app, "Sprints")
        src = app.page_source()
        assert name in src, f"Sprint '{name}' not in GUI"
        assert status in src, f"Status '{status}' not in GUI for sprint '{name}'"

    @staticmethod
    def assert_timer_state(app, *expected_texts: str) -> None:
        """Navigate to Timer tab and assert any of the expected texts appear."""
        from harness import click_tab
        click_tab(app, "Timer")
        body = app.text(app.find("body"))
        assert any(t in body for t in expected_texts), \
            f"Expected one of {expected_texts} in timer, got: {body[:200]}"


def get_uid(username):
    """Get user ID by username."""
    root = H()
    users = root.admin_users()
    for u in users:
        if u["username"] == username:
            return u["id"]
    raise ValueError(f"User '{username}' not found")
