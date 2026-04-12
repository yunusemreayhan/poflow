"""Pagination tests for list endpoints.

Verifies page/per_page params, x-total-count header, boundary values,
and filter consistency.
"""
import urllib.request
import json
import pytest
from helpers import H
import harness


def _raw_get(path, token):
    """GET with access to response headers."""
    req = urllib.request.Request(
        f"{harness.BASE_URL}{path}",
        headers={"Authorization": f"Bearer {token}", "X-Requested-With": "test"})
    resp = urllib.request.urlopen(req, timeout=10)
    body = json.loads(resp.read().decode())
    headers = dict(resp.headers)
    return resp.status, body, headers


_seeded = False


@pytest.fixture(autouse=True)
def _seed_once(logged_in):
    global _seeded
    if not _seeded:
        h = H()
        for i in range(50):
            h.create_task(f"Page_{i:03d}", project="Pagination",
                          priority=(i % 4) + 1)
        _seeded = True


class TestTaskPagination:

    def test_page1_returns_10(self, logged_in):
        h = H()
        _, tasks, hdrs = _raw_get("/api/tasks?page=1&per_page=10", h.token)
        assert len(tasks) == 10
        assert hdrs.get("x-total-count") is not None
        total = int(hdrs["x-total-count"])
        assert total >= 50

    def test_page2_different_from_page1(self, logged_in):
        h = H()
        _, p1, _ = _raw_get("/api/tasks?page=1&per_page=10", h.token)
        _, p2, _ = _raw_get("/api/tasks?page=2&per_page=10", h.token)
        assert len(p2) == 10
        p1_ids = {t["id"] for t in p1}
        p2_ids = {t["id"] for t in p2}
        assert p1_ids.isdisjoint(p2_ids)

    def test_last_page_correct_count(self, logged_in):
        h = H()
        _, _, hdrs = _raw_get("/api/tasks?page=1&per_page=10", h.token)
        total = int(hdrs["x-total-count"])
        last_page = (total + 9) // 10
        _, last, _ = _raw_get(f"/api/tasks?page={last_page}&per_page=10", h.token)
        expected = total - (last_page - 1) * 10
        assert len(last) == expected

    def test_beyond_last_page_empty(self, logged_in):
        h = H()
        _, _, hdrs = _raw_get("/api/tasks?page=1&per_page=10", h.token)
        total = int(hdrs["x-total-count"])
        far_page = (total // 10) + 100
        _, tasks, _ = _raw_get(f"/api/tasks?page={far_page}&per_page=10", h.token)
        assert len(tasks) == 0

    def test_total_count_header_accurate(self, logged_in):
        h = H()
        _, _, hdrs = _raw_get("/api/tasks?page=1&per_page=10", h.token)
        total = int(hdrs["x-total-count"])
        _, all_tasks, _ = _raw_get("/api/tasks?page=1&per_page=5000", h.token)
        assert total == len(all_tasks)

    def test_page_header_returned(self, logged_in):
        h = H()
        _, _, hdrs = _raw_get("/api/tasks?page=3&per_page=10", h.token)
        assert hdrs.get("x-page") == "3"
        assert hdrs.get("x-per-page") == "10"

    def test_no_page_param_returns_all(self, logged_in):
        h = H()
        _, tasks, hdrs = _raw_get("/api/tasks", h.token)
        assert isinstance(tasks, list)
        assert len(tasks) >= 50


class TestPaginationBoundary:

    def test_per_page_zero(self, logged_in):
        h = H()
        code, _ = h.api_status("GET", "/api/tasks?page=1&per_page=0")
        assert code in (200, 400)

    def test_per_page_negative(self, logged_in):
        h = H()
        code, _ = h.api_status("GET", "/api/tasks?page=1&per_page=-1")
        assert code in (200, 400, 422)

    def test_per_page_huge_clamped(self, logged_in):
        h = H()
        _, tasks, _ = _raw_get("/api/tasks?page=1&per_page=10000", h.token)
        assert len(tasks) <= 5000

    def test_page_zero_treated_as_1(self, logged_in):
        h = H()
        _, p0, _ = _raw_get("/api/tasks?page=0&per_page=10", h.token)
        _, p1, _ = _raw_get("/api/tasks?page=1&per_page=10", h.token)
        assert [t["id"] for t in p0] == [t["id"] for t in p1]

    def test_page_negative_treated_as_1(self, logged_in):
        h = H()
        _, pn, _ = _raw_get("/api/tasks?page=-5&per_page=10", h.token)
        _, p1, _ = _raw_get("/api/tasks?page=1&per_page=10", h.token)
        assert [t["id"] for t in pn] == [t["id"] for t in p1]

    def test_per_page_1(self, logged_in):
        h = H()
        _, tasks, hdrs = _raw_get("/api/tasks?page=1&per_page=1", h.token)
        assert len(tasks) == 1
        total = int(hdrs["x-total-count"])
        assert total >= 50


class TestFiltering:

    def test_default_sort_consistent(self, logged_in):
        h = H()
        _, r1, _ = _raw_get("/api/tasks?page=1&per_page=50", h.token)
        _, r2, _ = _raw_get("/api/tasks?page=1&per_page=50", h.token)
        assert [t["id"] for t in r1] == [t["id"] for t in r2]

    def test_filter_by_project(self, logged_in):
        h = H()
        _, tasks, _ = _raw_get("/api/tasks?project=Pagination&page=1&per_page=100", h.token)
        assert all(t["project"] == "Pagination" for t in tasks)
        assert len(tasks) == 50

    def test_filter_by_priority(self, logged_in):
        h = H()
        _, tasks, _ = _raw_get("/api/tasks?priority=1&page=1&per_page=100", h.token)
        assert all(t["priority"] == 1 for t in tasks)

    def test_filter_by_status(self, logged_in):
        h = H()
        _, tasks, _ = _raw_get("/api/tasks?status=backlog&page=1&per_page=5000", h.token)
        assert all(t["status"] == "backlog" for t in tasks)

    def test_combined_filters(self, logged_in):
        h = H()
        _, tasks, _ = _raw_get(
            "/api/tasks?project=Pagination&priority=2&page=1&per_page=100", h.token)
        for t in tasks:
            assert t["project"] == "Pagination"
            assert t["priority"] == 2
