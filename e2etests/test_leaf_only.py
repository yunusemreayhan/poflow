"""Test leaf_only_mode: config persistence, API returns all tasks, parent_id chain for breadcrumbs,
and that GUI-side filtering logic (replicated here) correctly filters parent tasks."""

import json, os, urllib.request, urllib.error
import pytest

@pytest.fixture(scope="module")
def daemon():
    import harness
    d = harness.Daemon()
    d.start()
    yield d
    d.stop()


def api(method, path, body=None, token=None, base_url=None):
    url = base_url or os.environ.get("POFLOW_TEST_URL", "http://127.0.0.1:9090")
    data = json.dumps(body).encode() if body is not None else (b"" if method in ("POST", "PUT") else None)
    hdrs = {"Content-Type": "application/json", "X-Requested-With": "test"}
    if token:
        hdrs["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(f"{url}{path}", data=data, headers=hdrs, method=method)
    try:
        resp = urllib.request.urlopen(req, timeout=10)
        raw = resp.read().decode()
        return resp.status, json.loads(raw) if raw else {}
    except urllib.error.HTTPError as e:
        raw = e.read().decode()
        try:
            return e.code, json.loads(raw)
        except Exception:
            return e.code, {"error": raw}


def login(base_url, user="root", pw=None):
    pw = pw or os.environ.get("POFLOW_ROOT_PASSWORD", "TestRoot1")
    status, body = api("POST", "/api/auth/login", {"username": user, "password": pw}, base_url=base_url)
    assert status == 200, f"Login failed: {body}"
    return body["token"]


def gui_leaf_filter(tasks):
    """Replicate the GUI-side leaf_only filtering logic from KanbanBoard.tsx / TaskList.tsx."""
    parent_ids = {t["parent_id"] for t in tasks if t.get("parent_id")}
    return [t for t in tasks if t["id"] not in parent_ids]


def build_ancestor_map(tasks):
    """Replicate the GUI-side ancestor breadcrumb logic from KanbanBoard.tsx."""
    by_id = {t["id"]: t for t in tasks}
    result = {}
    for t in tasks:
        path = []
        cur = by_id.get(t.get("parent_id")) if t.get("parent_id") else None
        while cur:
            path.insert(0, cur["title"])
            cur = by_id.get(cur.get("parent_id")) if cur.get("parent_id") else None
        if path:
            result[t["id"]] = path
    return result


class TestLeafOnlyMode:
    @pytest.fixture(autouse=True)
    def setup(self, daemon):
        self.url = daemon.base_url
        self.token = login(self.url)

        # 3-level hierarchy: grandparent -> parent -> child
        _, self.grandparent = api("POST", "/api/tasks", {"title": "Epic Project"}, self.token, self.url)
        _, self.parent = api("POST", "/api/tasks", {"title": "Backend Module", "parent_id": self.grandparent["id"]}, self.token, self.url)
        _, self.child = api("POST", "/api/tasks", {"title": "Fix auth bug", "parent_id": self.parent["id"]}, self.token, self.url)
        # Standalone leaf (no parent, no children)
        _, self.leaf = api("POST", "/api/tasks", {"title": "Quick fix"}, self.token, self.url)

    def _set_leaf_only(self, enabled):
        status, cfg = api("GET", "/api/config", token=self.token, base_url=self.url)
        assert status == 200
        cfg["leaf_only_mode"] = enabled
        status, result = api("PUT", "/api/config", cfg, self.token, self.url)
        assert status == 200
        assert result["leaf_only_mode"] == enabled

    def _get_tasks(self, endpoint="/api/tasks"):
        status, resp = api("GET", endpoint, token=self.token, base_url=self.url)
        assert status == 200
        return resp["tasks"] if "tasks" in resp else resp

    # --- Config persistence ---

    def test_config_default_leaf_only_false(self):
        """leaf_only_mode defaults to false."""
        status, cfg = api("GET", "/api/config", token=self.token, base_url=self.url)
        assert cfg["leaf_only_mode"] is False

    def test_config_toggle_persists(self):
        """Setting leaf_only_mode persists across GET calls."""
        self._set_leaf_only(True)
        status, cfg = api("GET", "/api/config", token=self.token, base_url=self.url)
        assert cfg["leaf_only_mode"] is True

        self._set_leaf_only(False)
        status, cfg = api("GET", "/api/config", token=self.token, base_url=self.url)
        assert cfg["leaf_only_mode"] is False

    def test_config_persists_across_relogin(self):
        """leaf_only_mode survives a fresh login."""
        self._set_leaf_only(True)
        token2 = login(self.url)
        status, cfg = api("GET", "/api/config", token=token2, base_url=self.url)
        assert cfg["leaf_only_mode"] is True
        # Clean up
        cfg["leaf_only_mode"] = False
        api("PUT", "/api/config", cfg, token2, self.url)

    # --- API returns all tasks regardless of leaf_only ---

    def test_api_tasks_returns_all_when_off(self):
        self._set_leaf_only(False)
        ids = [t["id"] for t in self._get_tasks()]
        assert self.grandparent["id"] in ids
        assert self.parent["id"] in ids
        assert self.child["id"] in ids
        assert self.leaf["id"] in ids

    def test_api_tasks_returns_all_when_on(self):
        """API must return ALL tasks even with leaf_only ON — filtering is GUI-side."""
        self._set_leaf_only(True)
        ids = [t["id"] for t in self._get_tasks()]
        assert self.grandparent["id"] in ids, "API must not filter grandparent"
        assert self.parent["id"] in ids, "API must not filter parent"
        assert self.child["id"] in ids
        assert self.leaf["id"] in ids

    def test_api_tasks_full_returns_all_when_on(self):
        """/api/tasks/full must also return all tasks."""
        self._set_leaf_only(True)
        ids = [t["id"] for t in self._get_tasks("/api/tasks/full")]
        assert self.grandparent["id"] in ids
        assert self.parent["id"] in ids
        assert self.child["id"] in ids
        assert self.leaf["id"] in ids

    # --- GUI-side leaf filtering (replicated logic) ---

    def test_gui_filter_excludes_parents(self):
        """Replicate GUI filtering: parents (tasks with children) are excluded."""
        tasks = self._get_tasks("/api/tasks/full")
        filtered = gui_leaf_filter(tasks)
        filtered_ids = [t["id"] for t in filtered]
        assert self.grandparent["id"] not in filtered_ids, "Grandparent has children — should be filtered"
        assert self.parent["id"] not in filtered_ids, "Parent has children — should be filtered"
        assert self.child["id"] in filtered_ids, "Child is a leaf — should remain"
        assert self.leaf["id"] in filtered_ids, "Standalone leaf — should remain"

    def test_gui_filter_keeps_all_when_no_hierarchy(self):
        """If no task has children, GUI filter keeps everything."""
        # Create two standalone tasks
        _, t1 = api("POST", "/api/tasks", {"title": "Solo A"}, self.token, self.url)
        _, t2 = api("POST", "/api/tasks", {"title": "Solo B"}, self.token, self.url)
        tasks = self._get_tasks("/api/tasks/full")
        filtered = gui_leaf_filter(tasks)
        filtered_ids = [t["id"] for t in filtered]
        assert t1["id"] in filtered_ids
        assert t2["id"] in filtered_ids

    # --- parent_id chain for ancestor breadcrumbs ---

    def test_parent_id_chain_intact(self):
        """Tasks carry parent_id so GUI can walk the ancestor chain."""
        tasks = self._get_tasks("/api/tasks/full")
        by_id = {t["id"]: t for t in tasks}
        child = by_id[self.child["id"]]
        parent = by_id[self.parent["id"]]
        grandparent = by_id[self.grandparent["id"]]
        assert child["parent_id"] == self.parent["id"]
        assert parent["parent_id"] == self.grandparent["id"]
        assert grandparent.get("parent_id") is None

    def test_ancestor_breadcrumb_deep(self):
        """Replicate GUI ancestor map: child should have ['Epic Project', 'Backend Module']."""
        tasks = self._get_tasks("/api/tasks/full")
        ancestors = build_ancestor_map(tasks)
        assert ancestors.get(self.child["id"]) == ["Epic Project", "Backend Module"]
        assert ancestors.get(self.parent["id"]) == ["Epic Project"]
        assert self.grandparent["id"] not in ancestors, "Root task has no ancestors"
        assert self.leaf["id"] not in ancestors, "Standalone leaf has no ancestors"

    def test_ancestor_breadcrumb_after_reparent(self):
        """Moving a task to a new parent updates the breadcrumb chain."""
        # Move child directly under grandparent (skip parent)
        api("PUT", f"/api/tasks/{self.child['id']}", {"parent_id": self.grandparent["id"]}, self.token, self.url)
        tasks = self._get_tasks("/api/tasks/full")
        ancestors = build_ancestor_map(tasks)
        assert ancestors.get(self.child["id"]) == ["Epic Project"], "After reparent, child's ancestor is just grandparent"

    # --- Creating children still works with leaf_only ON ---

    def test_can_create_child_with_leaf_only_on(self):
        """leaf_only must not prevent creating children under parents."""
        self._set_leaf_only(True)
        status, new_child = api("POST", "/api/tasks",
            {"title": "New subtask", "parent_id": self.parent["id"]}, self.token, self.url)
        assert status == 201, f"Should be able to create child task: {new_child}"
        assert new_child["parent_id"] == self.parent["id"]
