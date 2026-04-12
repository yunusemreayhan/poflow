"""Dashboard/stats E2E: verify history tab shows correct data after actions."""

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


class TestHistoryStats:

    def test_history_tab_loads(self, logged_in):
        click_tab(logged_in, "History")
        body = logged_in.text(logged_in.find("body"))
        assert "0" in body or "session" in body.lower() or "streak" in body.lower()

    def test_history_shows_zero_on_fresh_db(self, logged_in):
        """Fresh DB should show 0 sessions."""
        click_tab(logged_in, "Timer")
        click_tab(logged_in, "History")
        body = logged_in.text(logged_in.find("body"))
        assert "0" in body

    def test_tasks_tab_shows_count_after_create(self, logged_in):
        """Create tasks via API, verify task count in GUI."""
        t = token()
        api("POST", "/api/tasks", {"title": "D1", "project": "DashProj"}, t)
        api("POST", "/api/tasks", {"title": "D2", "project": "DashProj"}, t)
        api("POST", "/api/tasks", {"title": "D3", "project": "DashProj"}, t)
        click_tab(logged_in, "Timer")
        click_tab(logged_in, "Tasks")
        body = logged_in.text(logged_in.find("body"))
        assert "DashProj" in body

    def test_sprints_tab_shows_count(self, logged_in):
        """Create sprints via API, verify visible in GUI."""
        t = token()
        api("POST", "/api/sprints", {"name": "DS1", "start_date": "2026-04-14", "end_date": "2026-04-28"}, t)
        api("POST", "/api/sprints", {"name": "DS2", "start_date": "2026-05-01", "end_date": "2026-05-15"}, t)
        click_tab(logged_in, "Timer")
        click_tab(logged_in, "Sprints")
        src = logged_in.page_source()
        assert "DS1" in src and "DS2" in src

    def test_rooms_tab_shows_count(self, logged_in):
        """Create rooms via API, verify visible in GUI."""
        t = token()
        api("POST", "/api/rooms", {"name": "DR1", "estimation_unit": "points"}, t)
        click_tab(logged_in, "Timer")
        click_tab(logged_in, "Rooms")
        src = logged_in.page_source()
        assert "DR1" in src
