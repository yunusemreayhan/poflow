"""Settings flow E2E: change config via API, verify GUI reflects changes."""

import json, os, urllib.request
import pytest
import harness
from harness import ROOT_PASSWORD, click_tab, reload_and_login



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



class TestSettingsDisplay:

    def test_shows_work_duration(self, logged_in):
        click_tab(logged_in, "Settings")
        src = logged_in.page_source()
        assert "Work" in src

    def test_shows_estimation_mode(self, logged_in):
        click_tab(logged_in, "Settings")
        body = logged_in.text(logged_in.find("body"))
        assert "Estimation Mode" in body


class TestSettingsUpdate:

    def test_work_duration_change(self, logged_in):
        t = token()
        cfg = api("GET", "/api/config", token=t)
        cfg["work_duration_min"] = 42
        api("PUT", "/api/config", cfg, t)

        reload_and_login(logged_in)
        click_tab(logged_in, "Settings")
        val = logged_in.execute_js("""
            var inputs = document.querySelectorAll('input[type="number"]');
            for (var i = 0; i < inputs.length; i++)
                if (inputs[i].value === '42') return '42';
            return '';
        """)
        assert val == "42"

    def test_estimation_mode_change(self, logged_in):
        t = token()
        cfg = api("GET", "/api/config", token=t)
        cfg["estimation_mode"] = "hours"
        api("PUT", "/api/config", cfg, t)

        reload_and_login(logged_in)
        click_tab(logged_in, "Settings")
        src = logged_in.page_source()
        assert "hours" in src or "Hours" in src

    def test_config_persists_after_refresh(self, logged_in):
        t = token()
        cfg = api("GET", "/api/config", token=t)
        cfg["short_break_min"] = 7
        api("PUT", "/api/config", cfg, t)

        reload_and_login(logged_in)
        click_tab(logged_in, "Settings")
        val = logged_in.execute_js("""
            var inputs = document.querySelectorAll('input[type="number"]');
            for (var i = 0; i < inputs.length; i++)
                if (inputs[i].value === '7') return '7';
            return '';
        """)
        assert val == "7"
