"""Sprint lifecycle E2E: create via API, verify + interact via GUI."""

import time, json, os, urllib.request
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


def click_sprint(app, name):
    app.execute_js(f"""
        var d=document.querySelectorAll('div');
        for(var i=0;i<d.length;i++)
            if(d[i].textContent.includes('{name}')&&d[i].className.indexOf('cursor-pointer')>=0)
                {{d[i].click();break;}}
    """)
    # Wait for sprint detail to load
    deadline = time.time() + 5
    while time.time() < deadline:
        body = app.text(app.find("body"))
        if "Board" in body or "Todo" in body or "Complete" in body or "completed" in body:
            break
        time.sleep(0.3)


def nav_sprint(app, name):
    """Navigate away then back to Sprints to force data reload."""
    click_tab(app, "Timer")
    click_tab(app, "Sprints")
    click_sprint(app, name)


_ID = os.getpid()
NAME = f"Sprint_{_ID}"


@pytest.fixture(scope="module")
def sprint(daemon):
    t = token()
    t1 = api("POST", "/api/tasks", {"title": "ST1", "project": "SP"}, t)
    t2 = api("POST", "/api/tasks", {"title": "ST2", "project": "SP"}, t)
    s = api("POST", "/api/sprints", {"name": NAME, "start_date": "2026-04-14", "end_date": "2026-04-28"}, t)
    api("POST", f"/api/sprints/{s['id']}/tasks", {"task_ids": [t1["id"], t2["id"]]}, t)
    return s


class TestSprintDisplay:
    def test_visible_in_list(self, logged_in, sprint):
        click_tab(logged_in, "Refresh data")
        click_tab(logged_in, "Sprints")
        assert NAME in logged_in.page_source()

    def test_planning_status(self, logged_in, sprint):
        click_tab(logged_in, "Sprints")
        assert "planning" in logged_in.page_source()

    def test_detail_shows_board(self, logged_in, sprint):
        click_tab(logged_in, "Sprints")
        click_sprint(logged_in, NAME)
        body = logged_in.text(logged_in.find("body"))
        assert "Board" in body or "Todo" in body


class TestSprintLifecycle:
    def test_start(self, logged_in, sprint):
        t = token()
        s = api("GET", f"/api/sprints/{sprint['id']}", token=t)
        if s.get("sprint", {}).get("status") == "planning":
            api("POST", f"/api/sprints/{sprint['id']}/start", token=t)
        nav_sprint(logged_in, NAME)
        body = logged_in.text(logged_in.find("body"))
        assert "Complete" in body or "active" in body.lower()

    def test_active_columns(self, logged_in, sprint):
        nav_sprint(logged_in, NAME)
        body = logged_in.text(logged_in.find("body"))
        assert "Todo" in body and "In Progress" in body and "Done" in body

    def test_complete(self, logged_in, sprint):
        t = token()
        s = api("GET", f"/api/sprints/{sprint['id']}", token=t)
        if s.get("sprint", {}).get("status") == "active":
            api("POST", f"/api/sprints/{sprint['id']}/complete", token=t)
        nav_sprint(logged_in, NAME)
        assert "completed" in logged_in.page_source()

    def test_completed_in_list(self, logged_in, sprint):
        click_tab(logged_in, "Sprints")
        src = logged_in.page_source()
        assert NAME in src and "completed" in src
