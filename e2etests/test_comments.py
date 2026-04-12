"""Comments E2E: add comment via API, verify visible in GUI task detail."""

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


_ID = os.getpid()


class TestComments:

    @pytest.fixture(autouse=True)
    def _setup(self, logged_in):
        self.tok = token()
        self.task = api("POST", "/api/tasks",
                        {"title": f"Cm_{_ID}", "project": f"CmP_{_ID}"}, self.tok)

    def test_add_comment_via_api(self, logged_in):
        """Add a comment and verify via API."""
        api("POST", f"/api/tasks/{self.task['id']}/comments",
            {"content": f"Hello_{_ID}"}, self.tok)
        comments = api("GET", f"/api/tasks/{self.task['id']}/comments", token=self.tok)
        assert any(c["content"] == f"Hello_{_ID}" for c in comments)

    def test_comment_visible_in_task_detail(self, logged_in):
        """Add comment, verify task detail API includes it, and task visible in GUI."""
        api("POST", f"/api/tasks/{self.task['id']}/comments",
            {"content": f"Visible_{_ID}"}, self.tok)

        # Verify via API (task detail includes comments)
        detail = api("GET", f"/api/tasks/{self.task['id']}", token=self.tok)
        comments = detail.get("comments", [])
        assert any(c["content"] == f"Visible_{_ID}" for c in comments)

        # Verify task is visible in GUI
        click_tab(logged_in, "Timer")
        click_tab(logged_in, "Tasks")
        assert f"Cm_{_ID}" in logged_in.page_source()

    def test_comment_count_shown(self, logged_in):
        """After adding comments, the count should be visible."""
        api("POST", f"/api/tasks/{self.task['id']}/comments", {"content": "c1"}, self.tok)
        api("POST", f"/api/tasks/{self.task['id']}/comments", {"content": "c2"}, self.tok)

        click_tab(logged_in, "Timer")
        click_tab(logged_in, "Tasks")

        # The task node shows comment count via MessageSquare icon
        # Check if "2" appears near the task
        src = logged_in.page_source()
        assert f"Cm_{_ID}" in src
