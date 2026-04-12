"""Authorization matrix: verify every endpoint enforces auth correctly.

Parametrized tests that check unauthenticated access is rejected (401)
and wrong-method access returns 405.
"""
import pytest
from helpers import H, _api_status


# Endpoints that require auth (should return 401 without token)
AUTH_REQUIRED_ENDPOINTS = [
    ("GET", "/api/tasks"),
    ("POST", "/api/tasks"),
    ("GET", "/api/tasks/trash"),
    ("GET", "/api/tasks/search?q=test"),
    ("GET", "/api/sprints"),
    ("POST", "/api/sprints"),
    ("GET", "/api/rooms"),
    ("POST", "/api/rooms"),
    ("GET", "/api/labels"),
    ("POST", "/api/labels"),
    ("GET", "/api/teams"),
    ("POST", "/api/teams"),
    ("GET", "/api/timer"),
    ("GET", "/api/history"),
    ("GET", "/api/stats"),
    ("GET", "/api/config"),
    ("PUT", "/api/config"),
    ("GET", "/api/dependencies"),
    ("GET", "/api/assignees"),
    ("GET", "/api/burn-totals"),
    ("GET", "/api/reports/user-hours"),
    ("GET", "/api/admin/users"),
    ("POST", "/api/admin/backup"),
    ("GET", "/api/admin/backups"),
    ("GET", "/api/export/tasks"),
    ("POST", "/api/import/tasks/json"),
    ("POST", "/api/timer/start"),
    ("POST", "/api/timer/stop"),
    ("POST", "/api/timer/pause"),
    ("POST", "/api/timer/resume"),
    ("POST", "/api/timer/ticket"),
    ("PUT", "/api/profile"),
    ("GET", "/api/me/teams"),
    ("GET", "/api/audit"),
    ("GET", "/api/sprints/burndown"),
]

# Endpoints that do NOT require auth
PUBLIC_ENDPOINTS = [
    ("GET", "/api/health"),
]

# Wrong HTTP methods on known endpoints (should return 405)
WRONG_METHOD_ENDPOINTS = [
    ("DELETE", "/api/health"),
    ("PUT", "/api/health"),
    ("POST", "/api/health"),
    ("PATCH", "/api/tasks"),
    ("PATCH", "/api/sprints"),
    ("PATCH", "/api/rooms"),
    ("PATCH", "/api/labels"),
]


class TestAuthRequired:

    @pytest.mark.parametrize("method,path", AUTH_REQUIRED_ENDPOINTS)
    def test_unauthenticated_returns_401(self, logged_in, method, path):
        code, _ = _api_status(method, path)
        assert code == 401, f"{method} {path} returned {code}, expected 401"


class TestPublicEndpoints:

    @pytest.mark.parametrize("method,path", PUBLIC_ENDPOINTS)
    def test_public_returns_200(self, logged_in, method, path):
        code, _ = _api_status(method, path)
        assert code == 200, f"{method} {path} returned {code}, expected 200"


class TestWrongMethod:

    @pytest.mark.parametrize("method,path", WRONG_METHOD_ENDPOINTS)
    def test_wrong_method_returns_405(self, logged_in, method, path):
        h = H()
        code, _ = h.api_status(method, path)
        assert code == 405, f"{method} {path} returned {code}, expected 405"


# Admin-only endpoints that normal users can't access
ADMIN_ONLY_ENDPOINTS = [
    ("GET", "/api/admin/users"),
    ("POST", "/api/admin/backup"),
    ("GET", "/api/admin/backups"),
]


class TestAdminOnly:

    @pytest.mark.parametrize("method,path", ADMIN_ONLY_ENDPOINTS)
    def test_normal_user_gets_403(self, logged_in, method, path):
        user = H.register("authmatrix_user")
        code, _ = user.api_status(method, path)
        assert code == 403, f"{method} {path} returned {code}, expected 403"
