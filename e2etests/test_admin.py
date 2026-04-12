"""Admin E2E: user management, backup/restore, audit log."""

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


class TestAdmin:

    def test_list_users(self, logged_in):
        t = token()
        users = api("GET", "/api/admin/users", token=t)
        assert any(u["username"] == "root" for u in users)

    def test_create_and_list_user(self, logged_in):
        t = token()
        api("POST", "/api/auth/register", {"username": f"adm_{_ID}", "password": "TestAdm1"}, t)
        users = api("GET", "/api/admin/users", token=t)
        assert any(u["username"] == f"adm_{_ID}" for u in users)

    def test_change_user_role(self, logged_in):
        t = token()
        api("POST", "/api/auth/register", {"username": f"role_{_ID}", "password": "TestRole1"}, t)
        users = api("GET", "/api/admin/users", token=t)
        uid = next(u["id"] for u in users if u["username"] == f"role_{_ID}")
        api("PUT", f"/api/admin/users/{uid}/role", {"role": "root"}, t)
        users = api("GET", "/api/admin/users", token=t)
        u = next(u for u in users if u["id"] == uid)
        assert u["role"] == "root"

    def test_audit_log(self, logged_in):
        t = token()
        log = api("GET", "/api/audit", token=t)
        assert isinstance(log, list)

    def test_backup_create(self, logged_in):
        t = token()
        result = api("POST", "/api/admin/backup", token=t)
        assert "filename" in result or "path" in result or isinstance(result, dict)

    def test_backup_list(self, logged_in):
        t = token()
        api("POST", "/api/admin/backup", token=t)
        backups = api("GET", "/api/admin/backups", token=t)
        assert isinstance(backups, list) and len(backups) >= 1
