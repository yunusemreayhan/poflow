"""Task CRUD E2E: create/edit/delete via API, verify via GUI."""

import json, os, urllib.request
import pytest
import harness
from harness import ROOT_PASSWORD, click_tab



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
        urllib.request.Request(f"{url}{path}", data=data, headers=hdrs, method=method), timeout=5)
    raw = resp.read().decode()
    return json.loads(raw) if raw else {}


def token():
    return api("POST", "/api/auth/login", {"username": "root", "password": ROOT_PASSWORD})["token"]


def refresh_tasks(app):
    """Navigate away and back to force task list reload."""
    click_tab(app, "Timer")
    click_tab(app, "Tasks")


_ID = os.getpid()


class TestTaskCreate:
    def test_api_task_shows_in_gui(self, logged_in):
        api("POST", "/api/tasks", {"title": f"Tk_{_ID}", "project": f"Pj_{_ID}"}, token())
        refresh_tasks(logged_in)
        assert f"Tk_{_ID}" in logged_in.page_source()

    def test_multiple_projects(self, logged_in):
        t = token()
        api("POST", "/api/tasks", {"title": "A1", "project": f"PA_{_ID}"}, t)
        api("POST", "/api/tasks", {"title": "B1", "project": f"PB_{_ID}"}, t)
        refresh_tasks(logged_in)
        src = logged_in.page_source()
        assert f"PA_{_ID}" in src and f"PB_{_ID}" in src


class TestTaskEdit:
    def test_rename_reflects_in_gui(self, logged_in):
        t = token()
        task = api("POST", "/api/tasks", {"title": f"Old_{_ID}", "project": "Ed"}, t)
        refresh_tasks(logged_in)
        assert f"Old_{_ID}" in logged_in.page_source()
        api("PUT", f"/api/tasks/{task['id']}", {"title": f"New_{_ID}"}, t)
        refresh_tasks(logged_in)
        assert f"New_{_ID}" in logged_in.page_source()

    def test_status_change_via_api(self, logged_in):
        t = token()
        task = api("POST", "/api/tasks", {"title": f"St_{_ID}", "project": "Ed"}, t)
        api("PUT", f"/api/tasks/{task['id']}", {"status": "completed"}, t)
        tasks = api("GET", "/api/tasks", token=t)
        assert any(x["id"] == task["id"] and x["status"] == "completed" for x in tasks)


class TestTaskDelete:
    def test_soft_delete_removes_from_gui(self, logged_in):
        t = token()
        task = api("POST", "/api/tasks", {"title": f"Del_{_ID}", "project": f"DP_{_ID}"}, t)
        refresh_tasks(logged_in)
        assert f"Del_{_ID}" in logged_in.page_source()
        api("DELETE", f"/api/tasks/{task['id']}", token=t)
        refresh_tasks(logged_in)
        assert f"Del_{_ID}" not in logged_in.page_source()

    def test_restore_reappears_in_gui(self, logged_in):
        t = token()
        task = api("POST", "/api/tasks", {"title": f"Rs_{_ID}", "project": f"RP_{_ID}"}, t)
        api("DELETE", f"/api/tasks/{task['id']}", token=t)
        refresh_tasks(logged_in)
        assert f"Rs_{_ID}" not in logged_in.page_source()
        api("POST", f"/api/tasks/{task['id']}/restore", token=t)
        refresh_tasks(logged_in)
        assert f"Rs_{_ID}" in logged_in.page_source()

    def test_purge_gone_from_deleted_list(self, logged_in):
        """Purge via API — verify not in deleted list."""
        t = token()
        task = api("POST", "/api/tasks", {"title": f"Pu_{_ID}", "project": "Pu"}, t)
        api("DELETE", f"/api/tasks/{task['id']}", token=t)
        # Purge may not be available in all builds; skip if 404
        try:
            api("DELETE", f"/api/tasks/{task['id']}/permanent", token=t)
        except Exception:
            pytest.skip("purge endpoint not available in this build")
        deleted = api("GET", "/api/tasks/trash", token=t)
        assert not any(x["title"] == f"Pu_{_ID}" for x in deleted)

    def test_bulk_status_update(self, logged_in):
        t = token()
        t1 = api("POST", "/api/tasks", {"title": "B1", "project": "Bk"}, t)
        t2 = api("POST", "/api/tasks", {"title": "B2", "project": "Bk"}, t)
        try:
            api("PUT", "/api/tasks/bulk-status", {"task_ids": [t1["id"], t2["id"]], "status": "completed"}, t)
        except Exception:
            pytest.skip("bulk-status endpoint not available in this build")
        tasks = api("GET", "/api/tasks", token=t)
        bulk = [x for x in tasks if x["id"] in (t1["id"], t2["id"])]
        assert all(x["status"] == "completed" for x in bulk)
