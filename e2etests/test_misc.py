"""Time reporting, watchers, assignees, templates, notifications, password change."""

import time, json, os, urllib.request
import pytest
import harness
from harness import ROOT_PASSWORD

_ID = os.getpid()
_tok_cache = {}


def api(method, path, body=None, token=None):
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
        urllib.request.Request(f"{url}{path}", data=data, headers=hdrs, method=method), timeout=15)
    raw = resp.read().decode()
    return json.loads(raw) if raw else {}


def token(user="root", pw=ROOT_PASSWORD):
    if user not in _tok_cache:
        _tok_cache[user] = api("POST", "/api/auth/login", {"username": user, "password": pw})["token"]
    return _tok_cache[user]


class TestTimeReporting:

    def test_log_time_creates_entry(self, logged_in):
        t = token()
        task = api("POST", "/api/tasks", {"title": f"Tr_{_ID}", "project": "Tr"}, t)
        entry = api("POST", f"/api/tasks/{task['id']}/time", {"hours": 2.5}, t)
        assert entry["hours"] == 2.5

    def test_list_time_entries(self, logged_in):
        t = token()
        task = api("POST", "/api/tasks", {"title": f"TrL_{_ID}", "project": "Tr"}, t)
        api("POST", f"/api/tasks/{task['id']}/time", {"hours": 1.0}, t)
        entries = api("GET", f"/api/tasks/{task['id']}/time", token=t)
        assert len(entries) >= 1 and entries[0]["username"] == "root"


class TestWatchers:

    def test_watch_task(self, logged_in):
        t = token()
        task = api("POST", "/api/tasks", {"title": f"Wt_{_ID}", "project": "Wt"}, t)
        api("POST", f"/api/tasks/{task['id']}/watch", token=t)
        watchers = api("GET", f"/api/tasks/{task['id']}/watchers", token=t)
        assert "root" in watchers

    def test_watched_list(self, logged_in):
        t = token()
        task = api("POST", "/api/tasks", {"title": f"WtL_{_ID}", "project": "Wt"}, t)
        api("POST", f"/api/tasks/{task['id']}/watch", token=t)
        watched = api("GET", "/api/watched", token=t)
        assert task["id"] in watched


class TestAssignees:

    def test_assign_user(self, logged_in):
        t = token()
        task = api("POST", "/api/tasks", {"title": f"As_{_ID}", "project": "As"}, t)
        api("POST", f"/api/tasks/{task['id']}/assignees", {"username": "root"}, t)
        assignees = api("GET", f"/api/tasks/{task['id']}/assignees", token=t)
        assert "root" in assignees

    def test_remove_assignee(self, logged_in):
        t = token()
        task = api("POST", "/api/tasks", {"title": f"AsRm_{_ID}", "project": "As"}, t)
        api("POST", f"/api/tasks/{task['id']}/assignees", {"username": "root"}, t)
        api("DELETE", f"/api/tasks/{task['id']}/assignees/root", token=t)
        assignees = api("GET", f"/api/tasks/{task['id']}/assignees", token=t)
        assert "root" not in assignees


class TestTemplates:

    def test_create_template(self, logged_in):
        t = token()
        tpl = api("POST", "/api/templates", {"name": f"Tpl_{_ID}", "data": {"title": "X", "project": "P"}}, t)
        assert tpl["name"] == f"Tpl_{_ID}"

    def test_list_templates(self, logged_in):
        t = token()
        api("POST", "/api/templates", {"name": f"TplL_{_ID}", "data": {"title": "Y"}}, t)
        tpls = api("GET", "/api/templates", token=t)
        assert any(tp["name"] == f"TplL_{_ID}" for tp in tpls)

    def test_delete_template(self, logged_in):
        t = token()
        tpl = api("POST", "/api/templates", {"name": f"TplD_{_ID}", "data": {}}, t)
        api("DELETE", f"/api/templates/{tpl['id']}", token=t)
        tpls = api("GET", "/api/templates", token=t)
        assert not any(tp["id"] == tpl["id"] for tp in tpls)


class TestNotifications:

    def test_list_notifications(self, logged_in):
        t = token()
        notifs = api("GET", "/api/notifications", token=t)
        assert isinstance(notifs, list)


class TestPasswordChange:

    def test_change_password(self, logged_in):
        t = token()
        api("POST", "/api/auth/register", {"username": f"pw_{_ID}", "password": "OldPass1"}, t)
        t2 = token(f"pw_{_ID}", "OldPass1")
        api("PUT", "/api/auth/password", {"current_password": "OldPass1", "new_password": "NewPass1"}, t2)
        # Clear cache so next login uses new password
        _tok_cache.pop(f"pw_{_ID}", None)
        t3 = token(f"pw_{_ID}", "NewPass1")
        assert t3
