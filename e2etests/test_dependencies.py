"""Dependencies E2E: link tasks, verify via API and GUI."""

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


class TestDependencies:

    def test_add_dependency(self, logged_in):
        t = token()
        a = api("POST", "/api/tasks", {"title": f"DepA_{_ID}", "project": "Dep"}, t)
        b = api("POST", "/api/tasks", {"title": f"DepB_{_ID}", "project": "Dep"}, t)
        api("POST", f"/api/tasks/{a['id']}/dependencies", {"depends_on": b["id"]}, t)
        deps = api("GET", f"/api/tasks/{a['id']}/dependencies", token=t)
        assert b["id"] in deps or any(
            (d == b["id"] if isinstance(d, int) else d.get("depends_on") == b["id"]) for d in deps)

    def test_remove_dependency(self, logged_in):
        t = token()
        a = api("POST", "/api/tasks", {"title": f"RdA_{_ID}", "project": "Dep"}, t)
        b = api("POST", "/api/tasks", {"title": f"RdB_{_ID}", "project": "Dep"}, t)
        api("POST", f"/api/tasks/{a['id']}/dependencies", {"depends_on": b["id"]}, t)
        api("DELETE", f"/api/tasks/{a['id']}/dependencies/{b['id']}", token=t)
        deps = api("GET", f"/api/tasks/{a['id']}/dependencies", token=t)
        assert len(deps) == 0

    def test_dependency_graph_api(self, logged_in):
        t = token()
        deps = api("GET", "/api/dependencies", token=t)
        assert isinstance(deps, list)

    def test_dependent_tasks_in_gui(self, logged_in):
        t = token()
        api("POST", "/api/tasks", {"title": f"DpG_{_ID}", "project": f"DpGP_{_ID}"}, t)
        click_tab(logged_in, "Timer")
        click_tab(logged_in, "Tasks")
        assert f"DpG_{_ID}" in logged_in.page_source()
