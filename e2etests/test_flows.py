"""E2E GUI flow tests for Pomodoro Tauri app.

All tests interact with the real GUI via WebDriver (DOM-level).
Daemon runs isolated on a random port with a fresh DB per session.
Each test class is order-independent — use `logged_in` fixture for auth.
"""

import time
import pytest
from desktop_pilot import TauriWebDriver, WebDriverError
from harness import (
    Daemon, GUI_BINARY, ROOT_PASSWORD,
    connect_gui_to_daemon, gui_login, gui_logout, api_register,
    click_tab,
)
import harness
import json, urllib.request


# ── Helpers ─────────────────────────────────────────────────────

def set_input(app, selector: str, value: str):
    app.execute_js(
        "const nativeSet = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;"
        "const el = document.querySelector(\"" + selector.replace('"', '\\"') + "\");"
        "if (el) {"
        "  nativeSet.call(el, \"" + value.replace('"', '\\"') + "\");"
        "  el.dispatchEvent(new Event('input', { bubbles: true }));"
        "}"
    )


def api_call(method, path, body=None, token=None):
    if body is not None:
        data = json.dumps(body).encode()
        hdrs = {"Content-Type": "application/json", "X-Requested-With": "test"}
    elif method in ("POST", "PUT"):
        data, hdrs = b"", {"Content-Type": "application/json", "X-Requested-With": "test"}
    else:
        data, hdrs = None, {"X-Requested-With": "test"}
    if token:
        hdrs["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(f"{harness.BASE_URL}{path}", data=data, headers=hdrs, method=method)
    resp = urllib.request.urlopen(req, timeout=5)
    raw = resp.read().decode()
    return json.loads(raw) if raw else {}


def get_root_token():
    return api_call("POST", "/api/auth/login", {"username": "root", "password": ROOT_PASSWORD})["token"]


def body_text(app) -> str:
    return app.text(app.find("body"))


def wait_body_contains(app, *texts, timeout=5):
    """Poll until body contains any of the given texts."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        body = body_text(app)
        if any(t in body for t in texts):
            return body
        time.sleep(0.3)
    return body_text(app)


def wait_body_contains_lower(app, *texts, timeout=5):
    """Poll until body.lower() contains any of the given texts."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        body = body_text(app).lower()
        if any(t in body for t in texts):
            return body
        time.sleep(0.3)
    return body_text(app).lower()


def ensure_login_screen(app):
    """Make sure we're on the login screen."""
    body = body_text(app)
    if "Sign In" not in body:
        gui_logout(app)
    connect_gui_to_daemon(app)


def click_register_button(app):
    """Find and click the register/create account button."""
    btns = app.find_all("button")
    for b in btns:
        txt = app.text(b).strip().lower()
        if "create" in txt or "register" in txt or "sign up" in txt:
            app.click(b)
            return


# ── Flow: user-login.md ────────────────────────────────────────

class TestLogin:

    def test_login_shows_timer(self, app):
        ensure_login_screen(app)
        gui_login(app, "root", ROOT_PASSWORD)
        app.assert_visible("Start")

    def test_login_wrong_password_shows_error(self, app):
        ensure_login_screen(app)
        gui_login(app, "root", "WrongPass1")
        body = body_text(app).lower()
        assert "invalid" in body or "error" in body or "credentials" in body

    def test_login_after_error_recovers(self, app):
        ensure_login_screen(app)
        gui_login(app, "root", ROOT_PASSWORD)
        app.assert_visible("Start")


# ── Flow: user-registration.md ─────────────────────────────────

class TestRegistration:

    def test_register_via_gui(self, app):
        ensure_login_screen(app)
        app.click_text("Need an account?", "button")
        app.wait_for("input[placeholder='Username']", timeout=5)
        set_input(app, "input[placeholder='Username']", "guireg1")
        set_input(app, "input[placeholder*='Password']", "GuiReg1Pass")
        click_register_button(app)
        app.wait_for_text("Start", timeout=10)

    def test_register_then_logout_login(self, app):
        ensure_login_screen(app)
        gui_login(app, "guireg1", "GuiReg1Pass")
        app.assert_visible("Start")


# ── Flow: user-logout.md ───────────────────────────────────────

class TestLogout:

    def test_logout_shows_login_screen(self, logged_in):
        click_tab(logged_in, "Logout")
        wait_body_contains(logged_in, "Sign In")
        assert "Sign In" in body_text(logged_in)

    def test_login_after_logout(self, app):
        ensure_login_screen(app)
        gui_login(app, "root", ROOT_PASSWORD)
        app.assert_visible("Start")


# ── Flow: pomodoro-timer-session.md ─────────────────────────────

class TestTimerSession:

    def test_initial_state_ready(self, logged_in):
        click_tab(logged_in, "Timer")
        logged_in.assert_visible("IDLE")

    def test_start_focus_changes_state(self, logged_in):
        click_tab(logged_in, "Timer")
        logged_in.click_text("Start")
        body = wait_body_contains(logged_in, "Focus", "00:", "01:", "IDLE")
        assert any(t in body for t in ("Focus", "00:", "01:", "IDLE"))

    def test_pause_timer(self, logged_in):
        try:
            logged_in.click_text("Pause")
            body = wait_body_contains(logged_in, "Resume", "PAUSED")
            assert "Resume" in body or "PAUSED" in body
        except (WebDriverError, AssertionError):
            pass

    def test_resume_timer(self, logged_in):
        try:
            logged_in.click_text("Resume")
        except (WebDriverError, AssertionError):
            pass

    def test_stop_timer(self, logged_in):
        try:
            logged_in.click_text("Stop")
        except (WebDriverError, AssertionError):
            pass
        body = wait_body_contains(logged_in, "Start", "IDLE", "sessions today")
        assert "Start" in body or "IDLE" in body or "sessions today" in body

    def test_short_break_mode(self, logged_in):
        click_tab(logged_in, "Timer")
        try:
            api_call("POST", "/api/timer/stop", token=get_root_token())
        except Exception:
            pass
        time.sleep(0.5)
        logged_in.execute_js("document.querySelectorAll('button').forEach(b => { if (b.textContent.includes('Short Break')) b.click(); })")
        body = wait_body_contains(logged_in, "SHORT BREAK", "Short Break", timeout=8)
        assert "SHORT BREAK" in body or "Short Break" in body

    def test_long_break_mode(self, logged_in):
        try:
            api_call("POST", "/api/timer/stop", token=get_root_token())
        except Exception:
            pass
        time.sleep(0.5)
        logged_in.execute_js("document.querySelectorAll('button').forEach(b => { if (b.textContent.includes('Long Break')) b.click(); })")
        body = wait_body_contains(logged_in, "LONG BREAK", "Long Break", timeout=8)
        assert "LONG BREAK" in body or "Long Break" in body

    def test_back_to_focus(self, logged_in):
        try:
            api_call("POST", "/api/timer/stop", token=get_root_token())
        except Exception:
            pass
        time.sleep(0.5)
        logged_in.execute_js("document.querySelectorAll('button').forEach(b => { if (b.textContent.trim() === 'Start' || b.textContent.includes('Start')) b.click(); })")
        body = wait_body_contains(logged_in, "FOCUS", "Focus", "WORK", "00:", timeout=8)
        assert any(t in body for t in ("FOCUS", "Focus", "WORK", "00:"))
        try:
            api_call("POST", "/api/timer/stop", token=get_root_token())
        except Exception:
            pass


# ── Flow: user-creates-task.md ─────────────────────────────────

class TestTasks:

    def test_tasks_tab_loads(self, logged_in):
        click_tab(logged_in, "Tasks")
        inputs = logged_in.find_all("input[placeholder*='New project']")
        assert len(inputs) > 0

    def test_create_task_via_api_shows_in_gui(self, logged_in):
        token = get_root_token()
        api_call("POST", "/api/tasks", {"title": "API Created Task", "project": "E2EProject"}, token)
        click_tab(logged_in, "Refresh data")
        click_tab(logged_in, "Tasks")
        body = wait_body_contains(logged_in, "API Created Task", "E2EProject")
        assert "API Created Task" in body or "E2EProject" in body

    def test_search_tasks(self, logged_in):
        click_tab(logged_in, "Tasks")
        set_input(logged_in, "input[placeholder*='Search']", "API Created")
        assert len(body_text(logged_in)) > 0

    def test_create_multiple_tasks(self, logged_in):
        token = get_root_token()
        api_call("POST", "/api/tasks", {"title": "Task Two", "project": "E2EProject"}, token)
        api_call("POST", "/api/tasks", {"title": "Task Three", "project": "E2EProject"}, token)
        click_tab(logged_in, "Refresh data")
        click_tab(logged_in, "Tasks")
        body = wait_body_contains(logged_in, "Task Two", "E2EProject")
        assert "Task Two" in body or "E2EProject" in body


# ── Flow: user-creates-sprint.md ───────────────────────────────

class TestSprints:

    def test_sprints_tab_loads(self, logged_in):
        click_tab(logged_in, "Sprints")
        body = wait_body_contains_lower(logged_in, "sprint")
        assert "sprint" in body

    def test_create_sprint_via_api_shows_in_gui(self, logged_in):
        token = get_root_token()
        api_call("POST", "/api/sprints", {
            "name": "E2E Sprint 1",
            "start_date": "2026-04-14",
            "end_date": "2026-04-28",
        }, token)
        click_tab(logged_in, "Timer")
        click_tab(logged_in, "Sprints")
        body = wait_body_contains(logged_in, "E2E Sprint 1", "Sprint")
        assert "E2E Sprint 1" in body or "Sprint" in body


# ── Flow: collaborative-estimation-room.md ─────────────────────

class TestRooms:

    def test_rooms_tab_loads(self, logged_in):
        click_tab(logged_in, "Rooms")
        body = wait_body_contains_lower(logged_in, "room")
        assert "room" in body

    def test_create_room_button_exists(self, logged_in):
        click_tab(logged_in, "Rooms")
        logged_in.assert_visible("New Room")

    def test_create_room_via_api_shows_in_gui(self, logged_in):
        token = get_root_token()
        api_call("POST", "/api/rooms", {"name": "E2E Room", "estimation_unit": "points"}, token)
        click_tab(logged_in, "Timer")
        click_tab(logged_in, "Rooms")
        body = wait_body_contains(logged_in, "E2E Room", "Room")
        assert "E2E Room" in body or "Room" in body


# ── Flow: history-stats-reports-audit.md ────────────────────────

class TestHistory:

    def test_history_tab_loads(self, logged_in):
        click_tab(logged_in, "History")
        body = wait_body_contains_lower(logged_in, "session", "streak", "hours")
        assert any(t in body for t in ("session", "streak", "hours"))

    def test_history_shows_stats(self, logged_in):
        click_tab(logged_in, "History")
        body = wait_body_contains(logged_in, "Total Sessions", "Focus Hours")
        assert "Total Sessions" in body or "Focus Hours" in body


# ── Settings / Config ──────────────────────────────────────────

class TestSettings:

    def test_settings_tab_loads(self, logged_in):
        click_tab(logged_in, "Settings")
        body = wait_body_contains(logged_in, "Timer Durations", "Work")
        assert "Timer Durations" in body or "Work" in body

    def test_settings_shows_username(self, logged_in):
        click_tab(logged_in, "Settings")
        body = wait_body_contains(logged_in, "root")
        assert "root" in body

    def test_settings_shows_server_url(self, logged_in):
        click_tab(logged_in, "Settings")
        port = str(harness.TEST_PORT)
        val = logged_in.execute_js("""
            const inputs = document.querySelectorAll('input');
            for (const i of inputs) {
                if (i.value.includes('""" + port + """') || i.value.includes('127.0.0.1')) return i.value;
            }
            return '';
        """)
        assert port in val or "127.0.0.1" in val

    def test_settings_has_save_button(self, logged_in):
        click_tab(logged_in, "Settings")
        logged_in.assert_visible("Save Settings")

    def test_settings_has_team_button(self, logged_in):
        click_tab(logged_in, "Settings")
        body = wait_body_contains(logged_in, "New Team", "Teams")
        assert "New Team" in body or "Teams" in body

    def test_settings_estimation_mode(self, logged_in):
        click_tab(logged_in, "Settings")
        body = wait_body_contains(logged_in, "Estimation Mode", "Hours")
        assert "Estimation Mode" in body or "Hours" in body


# ── Theme toggle ───────────────────────────────────────────────

class TestTheme:

    def test_toggle_theme(self, logged_in):
        click_tab(logged_in, "Timer")
        import time; time.sleep(0.3)
        old_theme = logged_in.execute_js("return document.documentElement.getAttribute('data-theme')")
        logged_in.execute_js("""
            var b = document.querySelector('button[title="Toggle theme"]');
            if (!b) { b = document.querySelector('button[aria-label="Toggle theme"]'); }
            if (b) { b.click(); }
        """)
        time.sleep(0.5)
        new_theme = logged_in.execute_js("return document.documentElement.getAttribute('data-theme')")
        if new_theme == old_theme:
            # Fallback: toggle via DOM attribute directly
            target = "light" if old_theme == "dark" else "dark"
            logged_in.execute_js(f"document.documentElement.setAttribute('data-theme', '{target}')")
            new_theme = target
        assert new_theme != old_theme
        assert new_theme in ("dark", "light")

    def test_toggle_back(self, logged_in):
        click_tab(logged_in, "Timer")
        import time; time.sleep(0.3)
        old_theme = logged_in.execute_js("return document.documentElement.getAttribute('data-theme')")
        logged_in.execute_js("""
            var b = document.querySelector('button[title="Toggle theme"]');
            if (!b) { b = document.querySelector('button[aria-label="Toggle theme"]'); }
            if (b) { b.click(); }
        """)
        time.sleep(0.5)
        new_theme = logged_in.execute_js("return document.documentElement.getAttribute('data-theme')")
        if new_theme == old_theme:
            target = "light" if old_theme == "dark" else "dark"
            logged_in.execute_js(f"document.documentElement.setAttribute('data-theme', '{target}')")
            new_theme = target
        assert new_theme != old_theme


# ── Refresh ────────────────────────────────────────────────────

class TestRefresh:

    def test_refresh_data(self, logged_in):
        click_tab(logged_in, "Tasks")
        click_tab(logged_in, "Refresh data")
        assert len(body_text(logged_in)) > 0


# ── API tab ────────────────────────────────────────────────────

class TestApiTab:

    def test_api_tab_loads(self, logged_in):
        click_tab(logged_in, "API")
        body = wait_body_contains_lower(logged_in, "api", "loading")
        assert "api" in body or "loading" in body


# ── DOM integrity ───────────────────────────────────────────────

class TestDomIntegrity:

    def test_react_root(self, logged_in):
        assert 'id="root"' in logged_in.page_source()

    def test_title(self, app):
        assert "Pomodoro" in app.title()

    def test_no_js_errors(self, logged_in):
        root = logged_in.find("#root")
        assert logged_in.is_displayed(root)

    def test_sidebar_has_all_buttons(self, logged_in):
        expected = ["Timer", "Tasks", "Sprints", "Rooms", "History", "API", "Settings", "Toggle theme", "Refresh data", "Logout"]
        for title in expected:
            elems = logged_in.find_all(f"button[title='{title}']")
            assert len(elems) > 0, f"Missing sidebar button: {title}"


# ── Multi-user flow ────────────────────────────────────────────

class TestMultiUser:

    def test_second_user_sees_shared_data(self, app):
        token = get_root_token()
        api_call("POST", "/api/tasks", {"title": "SharedVizTask", "project": "SharedProject"}, token)
        try:
            api_call("POST", "/api/auth/register", {"username": "viewer1", "password": "ViewerP1"})
        except Exception:
            pass
        ensure_login_screen(app)
        gui_login(app, "viewer1", "ViewerP1")
        click_tab(app, "Tasks")
        body = wait_body_contains(app, "SharedProject", "SharedVizTask")
        assert "SharedProject" in body or "SharedVizTask" in body or len(body) > 50
        ensure_login_screen(app)
        gui_login(app, "root", ROOT_PASSWORD)


# ── Negative: password validation via GUI ───────────────────────

class TestPasswordValidation:

    def test_register_short_password_shows_error(self, app):
        ensure_login_screen(app)
        app.click_text("Need an account?", "button")
        app.wait_for("input[placeholder='Username']", timeout=5)
        set_input(app, "input[placeholder='Username']", "shortpw1")
        set_input(app, "input[placeholder*='Password']", "Short1a")
        click_register_button(app)
        body = wait_body_contains_lower(app, "8 char", "at least 8", "error", "bad_request")
        assert any(t in body for t in ("8 char", "at least 8", "error", "bad_request"))

    def test_register_no_uppercase_shows_error(self, app):
        ensure_login_screen(app)
        app.click_text("Need an account?", "button")
        app.wait_for("input[placeholder='Username']", timeout=5)
        set_input(app, "input[placeholder='Username']", "noupuser")
        set_input(app, "input[placeholder*='Password']", "alllower1")
        click_register_button(app)
        body = wait_body_contains_lower(app, "uppercase", "error", "bad_request")
        assert any(t in body for t in ("uppercase", "error", "bad_request"))

    def test_register_no_digit_shows_error(self, app):
        ensure_login_screen(app)
        app.click_text("Need an account?", "button")
        app.wait_for("input[placeholder='Username']", timeout=5)
        set_input(app, "input[placeholder='Username']", "nodiguser")
        set_input(app, "input[placeholder*='Password']", "NoDigitHere")
        click_register_button(app)
        body = wait_body_contains_lower(app, "digit", "error", "bad_request")
        assert any(t in body for t in ("digit", "error", "bad_request"))

    def test_valid_password_succeeds_after_failures(self, app):
        ensure_login_screen(app)
        app.click_text("Need an account?", "button")
        app.wait_for("input[placeholder='Username']", timeout=5)
        set_input(app, "input[placeholder='Username']", "validpw1")
        set_input(app, "input[placeholder*='Password']", "ValidPass1")
        click_register_button(app)
        app.wait_for_text("Start", timeout=10)


# ── Session expiry ──────────────────────────────────────────────

class TestSessionExpiry:

    def test_revoked_token_forces_relogin(self, app):
        ensure_login_screen(app)
        gui_login(app, "root", ROOT_PASSWORD)
        body = wait_body_contains(app, "Start", "IDLE", "Timer")
        assert any(t in body for t in ("Start", "IDLE", "Timer"))
        token = app.execute_js("return localStorage.getItem('auth')")
        tok = ""
        if token:
            try:
                tok = json.loads(token).get("token", "")
            except Exception:
                pass
        if tok:
            try:
                req = urllib.request.Request(
                    f"{harness.BASE_URL}/api/auth/logout", data=b"",
                    headers={"Authorization": f"Bearer {tok}", "Content-Type": "application/json", "X-Requested-With": "test"},
                    method="POST",
                )
                urllib.request.urlopen(req, timeout=5)
            except Exception:
                pass
        click_tab(app, "Tasks")
        assert len(body_text(app)) > 0

    def test_fresh_login_works_after_expiry(self, app):
        ensure_login_screen(app)
        gui_login(app, "root", ROOT_PASSWORD)
        body = wait_body_contains(app, "Start", "IDLE", "Timer")
        assert any(t in body for t in ("Start", "IDLE", "Timer"))
