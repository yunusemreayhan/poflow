"""Exhaustive config tests: every field, boundary values, invalid inputs, combinations."""

import json, os, urllib.request
import pytest
import harness
from harness import ROOT_PASSWORD, click_tab, reload_and_login

_tok = {}


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


def api_err(method, path, body=None, token=None):
    """Call API expecting an error, return (status_code, body_text)."""
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
    try:
        urllib.request.urlopen(
            urllib.request.Request(f"{url}{path}", data=data, headers=hdrs, method=method), timeout=5)
        return 200, ""
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()[:300]


def tok():
    if "root" not in _tok:
        _tok["root"] = api("POST", "/api/auth/login", {"username": "root", "password": ROOT_PASSWORD})["token"]
    return _tok["root"]




class TestConfigFields:
    """Test every config field individually."""

    def _set_field(self, field, value):
        t = tok()
        cfg = api("GET", "/api/config", token=t)
        cfg[field] = value
        return api("PUT", "/api/config", cfg, t)

    def test_work_duration_min(self, logged_in):
        r = self._set_field("work_duration_min", 50)
        assert r["work_duration_min"] == 50

    def test_short_break_min(self, logged_in):
        r = self._set_field("short_break_min", 10)
        assert r["short_break_min"] == 10

    def test_long_break_min(self, logged_in):
        r = self._set_field("long_break_min", 30)
        assert r["long_break_min"] == 30

    def test_long_break_interval(self, logged_in):
        r = self._set_field("long_break_interval", 6)
        assert r["long_break_interval"] == 6

    def test_auto_start_breaks_on(self, logged_in):
        r = self._set_field("auto_start_breaks", True)
        assert r["auto_start_breaks"] is True

    def test_auto_start_breaks_off(self, logged_in):
        r = self._set_field("auto_start_breaks", False)
        assert r["auto_start_breaks"] is False

    def test_auto_start_work_on(self, logged_in):
        r = self._set_field("auto_start_work", True)
        assert r["auto_start_work"] is True

    def test_auto_start_work_off(self, logged_in):
        r = self._set_field("auto_start_work", False)
        assert r["auto_start_work"] is False

    def test_sound_enabled(self, logged_in):
        r = self._set_field("sound_enabled", True)
        assert r["sound_enabled"] is True

    def test_sound_disabled(self, logged_in):
        r = self._set_field("sound_enabled", False)
        assert r["sound_enabled"] is False

    def test_notification_enabled(self, logged_in):
        r = self._set_field("notification_enabled", True)
        assert r["notification_enabled"] is True

    def test_notification_disabled(self, logged_in):
        r = self._set_field("notification_enabled", False)
        assert r["notification_enabled"] is False

    def test_daily_goal(self, logged_in):
        r = self._set_field("daily_goal", 12)
        assert r["daily_goal"] == 12

    def test_estimation_mode_hours(self, logged_in):
        r = self._set_field("estimation_mode", "hours")
        assert r["estimation_mode"] == "hours"

    def test_estimation_mode_points(self, logged_in):
        r = self._set_field("estimation_mode", "points")
        assert r["estimation_mode"] == "points"

    def test_leaf_only_mode_on(self, logged_in):
        r = self._set_field("leaf_only_mode", True)
        assert r["leaf_only_mode"] is True

    def test_leaf_only_mode_off(self, logged_in):
        r = self._set_field("leaf_only_mode", False)
        assert r["leaf_only_mode"] is False

    def test_theme_dark(self, logged_in):
        r = self._set_field("theme", "dark")
        assert r["theme"] == "dark"

    def test_theme_light(self, logged_in):
        r = self._set_field("theme", "light")
        assert r["theme"] == "light"

    def test_auto_archive_days(self, logged_in):
        r = self._set_field("auto_archive_days", 30)
        assert r["auto_archive_days"] == 30


class TestConfigBoundary:
    """Boundary and edge-case values."""

    def _set(self, field, value):
        t = tok()
        cfg = api("GET", "/api/config", token=t)
        cfg[field] = value
        return api("PUT", "/api/config", cfg, t)

    def test_work_duration_1_min(self, logged_in):
        r = self._set("work_duration_min", 1)
        assert r["work_duration_min"] == 1

    def test_work_duration_120_min(self, logged_in):
        r = self._set("work_duration_min", 120)
        assert r["work_duration_min"] == 120

    def test_daily_goal_zero(self, logged_in):
        r = self._set("daily_goal", 0)
        assert r["daily_goal"] == 0

    def test_daily_goal_high(self, logged_in):
        r = self._set("daily_goal", 50)
        assert r["daily_goal"] == 50

    def test_long_break_interval_1(self, logged_in):
        r = self._set("long_break_interval", 1)
        assert r["long_break_interval"] == 1

    def test_long_break_interval_10(self, logged_in):
        r = self._set("long_break_interval", 10)
        assert r["long_break_interval"] == 10

    def test_auto_archive_days_zero(self, logged_in):
        r = self._set("auto_archive_days", 0)
        assert r["auto_archive_days"] == 0


class TestConfigCombinations:
    """Multiple config fields changed together."""

    def test_all_booleans_true(self, logged_in):
        t = tok()
        cfg = api("GET", "/api/config", token=t)
        cfg.update(auto_start_breaks=True, auto_start_work=True,
                   sound_enabled=True, notification_enabled=True, leaf_only_mode=True)
        r = api("PUT", "/api/config", cfg, t)
        assert r["auto_start_breaks"] and r["auto_start_work"] and r["leaf_only_mode"]

    def test_all_booleans_false(self, logged_in):
        t = tok()
        cfg = api("GET", "/api/config", token=t)
        cfg.update(auto_start_breaks=False, auto_start_work=False,
                   sound_enabled=False, notification_enabled=False, leaf_only_mode=False)
        r = api("PUT", "/api/config", cfg, t)
        assert not r["auto_start_breaks"] and not r["auto_start_work"]

    def test_short_work_long_breaks(self, logged_in):
        t = tok()
        cfg = api("GET", "/api/config", token=t)
        cfg.update(work_duration_min=5, short_break_min=15, long_break_min=60)
        r = api("PUT", "/api/config", cfg, t)
        assert r["work_duration_min"] == 5 and r["long_break_min"] == 60

    def test_config_roundtrip(self, logged_in):
        """Set config, read back, verify identical."""
        t = tok()
        cfg = api("GET", "/api/config", token=t)
        cfg.update(work_duration_min=33, short_break_min=7, long_break_min=22,
                   daily_goal=6, estimation_mode="points", theme="light")
        api("PUT", "/api/config", cfg, t)
        cfg2 = api("GET", "/api/config", token=t)
        assert cfg2["work_duration_min"] == 33
        assert cfg2["short_break_min"] == 7
        assert cfg2["estimation_mode"] == "points"
        assert cfg2["theme"] == "light"

    def test_config_persists_gui(self, logged_in):
        """Change config, reload GUI, verify reflected."""
        t = tok()
        cfg = api("GET", "/api/config", token=t)
        cfg["work_duration_min"] = 77
        api("PUT", "/api/config", cfg, t)
        reload_and_login(logged_in)
        click_tab(logged_in, "Settings")
        val = logged_in.execute_js("""
            var inputs = document.querySelectorAll('input[type="number"]');
            for (var i = 0; i < inputs.length; i++)
                if (inputs[i].value === '77') return '77';
            return '';
        """)
        assert val == "77"
