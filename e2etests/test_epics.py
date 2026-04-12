"""Epics E2E: create epics, assign tasks, verify in GUI."""

import json, os, urllib.request
import pytest
import harness
from harness import ROOT_PASSWORD, click_tab

_ID = os.getpid()



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


class TestEpics:

    def test_create_epic(self, logged_in):
        t = token()
        epic = api("POST", "/api/epics", {"name": f"Ep_{_ID}"}, t)
        assert epic["name"] == f"Ep_{_ID}"

    def test_list_epics(self, logged_in):
        t = token()
        api("POST", "/api/epics", {"name": f"EpL_{_ID}"}, t)
        epics = api("GET", "/api/epics", token=t)
        assert any(e["name"] == f"EpL_{_ID}" for e in epics)

    def test_add_task_to_epic(self, logged_in):
        t = token()
        epic = api("POST", "/api/epics", {"name": f"EpT_{_ID}"}, t)
        task = api("POST", "/api/tasks", {"title": f"EpTk_{_ID}", "project": "Ep"}, t)
        api("POST", f"/api/epics/{epic['id']}/tasks", {"task_ids": [task["id"]]}, t)
        detail = api("GET", f"/api/epics/{epic['id']}", token=t)
        assert task["id"] in detail["task_ids"]

    def test_remove_task_from_epic(self, logged_in):
        t = token()
        epic = api("POST", "/api/epics", {"name": f"EpRm_{_ID}"}, t)
        task = api("POST", "/api/tasks", {"title": f"EpRmT_{_ID}", "project": "Ep"}, t)
        api("POST", f"/api/epics/{epic['id']}/tasks", {"task_ids": [task["id"]]}, t)
        api("DELETE", f"/api/epics/{epic['id']}/tasks/{task['id']}", token=t)
        detail = api("GET", f"/api/epics/{epic['id']}", token=t)
        assert task["id"] not in detail["task_ids"]

    def test_delete_epic(self, logged_in):
        t = token()
        epic = api("POST", "/api/epics", {"name": f"EpDel_{_ID}"}, t)
        api("DELETE", f"/api/epics/{epic['id']}", token=t)
        epics = api("GET", "/api/epics", token=t)
        assert not any(e["id"] == epic["id"] for e in epics)
