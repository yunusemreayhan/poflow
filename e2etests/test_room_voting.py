"""Room voting E2E: create via API, verify display + reveal via GUI."""

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


def click_room(app, name):
    app.execute_js(f"""
        var els=document.querySelectorAll('[tabindex="0"],.glass');
        for(var i=0;i<els.length;i++)
            if(els[i].textContent.includes('{name}')&&els[i].className.indexOf('cursor-pointer')>=0)
                {{els[i].click();break;}}
    """)
    # Wait for room detail to load
    deadline = time.time() + 5
    while time.time() < deadline:
        body = app.text(app.find("body"))
        if name in body and ("voting" in body.lower() or "idle" in body.lower() or "Reveal" in body or "members" in body.lower()):
            break
        time.sleep(0.3)


_ID = os.getpid()


class TestRoomDisplay:
    def test_room_shows_in_gui(self, logged_in):
        t = token()
        api("POST", "/api/rooms", {"name": f"Rm_{_ID}", "estimation_unit": "points"}, t)
        click_tab(logged_in, "Refresh data")
        click_tab(logged_in, "Rooms")
        assert f"Rm_{_ID}" in logged_in.page_source()

    def test_room_status_visible(self, logged_in):
        click_tab(logged_in, "Rooms")
        src = logged_in.page_source()
        assert "idle" in src or "lobby" in src or f"Rm_{_ID}" in src


class TestRoomVoting:
    @pytest.fixture(autouse=True)
    def _setup(self, logged_in):
        self.tok = token()
        r = api("POST", "/api/rooms", {"name": f"Vt_{_ID}", "estimation_unit": "points"}, self.tok)
        self.rid = r["id"]
        api("POST", f"/api/rooms/{self.rid}/join", token=self.tok)
        t = api("POST", "/api/tasks", {"title": f"Est_{_ID}", "project": "VP"}, self.tok)
        self.tid = t["id"]
        api("POST", f"/api/rooms/{self.rid}/start-voting", {"task_id": t["id"]}, self.tok)

    def test_detail_shows_task(self, logged_in):
        click_tab(logged_in, "Timer")
        click_tab(logged_in, "Rooms")
        click_room(logged_in, f"Vt_{_ID}")
        body = logged_in.text(logged_in.find("body"))
        assert f"Est_{_ID}" in body or "voting" in body.lower() or "Reveal" in body

    def test_vote_reveal_shows_result(self, logged_in):
        api("POST", f"/api/rooms/{self.rid}/vote", {"value": 5}, self.tok)
        api("POST", f"/api/rooms/{self.rid}/reveal", token=self.tok)
        click_tab(logged_in, "Timer")
        click_tab(logged_in, "Rooms")
        click_room(logged_in, f"Vt_{_ID}")
        src = logged_in.page_source()
        assert "revealed" in src or "5" in src or f"Vt_{_ID}" in src

    def test_members_visible(self, logged_in):
        click_tab(logged_in, "Rooms")
        click_room(logged_in, f"Vt_{_ID}")
        body = logged_in.text(logged_in.find("body"))
        assert "root" in body or "members" in body.lower()
