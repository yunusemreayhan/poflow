"""Labels E2E: CRUD labels, assign to tasks, verify in GUI."""

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


class TestLabels:

    def test_create_label(self, logged_in):
        t = token()
        lbl = api("POST", "/api/labels", {"name": f"Lb_{_ID}", "color": "#ff0000"}, t)
        assert lbl["name"] == f"Lb_{_ID}"

    def test_list_labels(self, logged_in):
        t = token()
        api("POST", "/api/labels", {"name": f"Lb2_{_ID}", "color": "#00ff00"}, t)
        labels = api("GET", "/api/labels", token=t)
        assert any(l["name"] == f"Lb2_{_ID}" for l in labels)

    def test_assign_label_to_task(self, logged_in):
        t = token()
        lbl = api("POST", "/api/labels", {"name": f"Asgn_{_ID}", "color": "#0000ff"}, t)
        task = api("POST", "/api/tasks", {"title": f"LbT_{_ID}", "project": "LbP"}, t)
        api("PUT", f"/api/tasks/{task['id']}/labels/{lbl['id']}", token=t)
        labels = api("GET", f"/api/tasks/{task['id']}/labels", token=t)
        assert any(l["id"] == lbl["id"] for l in labels)

    def test_remove_label_from_task(self, logged_in):
        t = token()
        lbl = api("POST", "/api/labels", {"name": f"Rm_{_ID}", "color": "#ff00ff"}, t)
        task = api("POST", "/api/tasks", {"title": f"RmT_{_ID}", "project": "LbP"}, t)
        api("PUT", f"/api/tasks/{task['id']}/labels/{lbl['id']}", token=t)
        api("DELETE", f"/api/tasks/{task['id']}/labels/{lbl['id']}", token=t)
        labels = api("GET", f"/api/tasks/{task['id']}/labels", token=t)
        assert not any(l["id"] == lbl["id"] for l in labels)

    def test_delete_label(self, logged_in):
        t = token()
        lbl = api("POST", "/api/labels", {"name": f"Del_{_ID}", "color": "#aabbcc"}, t)
        api("DELETE", f"/api/labels/{lbl['id']}", token=t)
        labels = api("GET", "/api/labels", token=t)
        assert not any(l["id"] == lbl["id"] for l in labels)

    def test_labeled_task_visible_in_gui(self, logged_in):
        t = token()
        api("POST", "/api/labels", {"name": f"Gui_{_ID}", "color": "#ff5500"}, t)
        api("POST", "/api/tasks", {"title": f"LbGui_{_ID}", "project": f"LbGP_{_ID}"}, t)
        # Force GUI to re-fetch tasks from server
        logged_in.execute_js("window.__STORE__?.getState?.()?.loadTasks?.()")
        click_tab(logged_in, "Refresh data")
        click_tab(logged_in, "Tasks")
        import time; time.sleep(2)
        body = logged_in.execute_js("return document.body.innerText || ''")
        assert f"LbGui_{_ID}" in body
