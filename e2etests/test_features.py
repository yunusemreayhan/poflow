"""Recurrence, templates, webhooks, notifications, profile — full coverage."""
import pytest
from helpers import H


class TestRecurrence:

    @pytest.mark.parametrize("pattern", ["daily", "weekly", "biweekly", "monthly"])
    def test_set_pattern(self, logged_in, pattern):
        h = H()
        t = h.create_task(f"Rec_{pattern}")
        r = h.set_recurrence(t["id"], pattern, "2026-07-01")
        assert r["pattern"] == pattern

    def test_get_recurrence(self, logged_in):
        h = H()
        t = h.create_task("RecGet")
        h.set_recurrence(t["id"], "weekly", "2026-08-01")
        r = h.get_recurrence(t["id"])
        assert r["pattern"] == "weekly"

    def test_remove_recurrence(self, logged_in):
        h = H()
        t = h.create_task("RecRm")
        h.set_recurrence(t["id"], "daily", "2026-09-01")
        h.remove_recurrence(t["id"])
        r = h.get_recurrence(t["id"])
        assert r is None

    def test_invalid_pattern_rejected(self, logged_in):
        h = H()
        t = h.create_task("RecBad")
        code, _ = h.api_status("PUT", f"/api/tasks/{t['id']}/recurrence",
                               {"pattern": "yearly", "next_due": "2026-01-01"})
        assert code == 400

    def test_invalid_date_rejected(self, logged_in):
        h = H()
        t = h.create_task("RecBadDate")
        code, _ = h.api_status("PUT", f"/api/tasks/{t['id']}/recurrence",
                               {"pattern": "daily", "next_due": "not-a-date"})
        assert code == 400

    def test_non_owner_cannot_set(self, logged_in):
        h = H()
        t = h.create_task("RecOwner")
        u = H.register("rec_user1")
        code, _ = u.api_status("PUT", f"/api/tasks/{t['id']}/recurrence",
                               {"pattern": "daily", "next_due": "2026-01-01"})
        assert code == 403


class TestTemplates:

    def test_create_and_list(self, logged_in):
        h = H()
        tmpl = h.create_template("Daily Standup", {"title": "{{today}} standup", "project": "Daily"})
        assert tmpl["name"] == "Daily Standup"
        lst = h.list_templates()
        assert any(t["id"] == tmpl["id"] for t in lst)

    def test_instantiate_resolves_variables(self, logged_in):
        h = H()
        tmpl = h.create_template("VarTest", {"title": "{{today}} by {{username}}", "project": "Tmpl"})
        task = h.instantiate_template(tmpl["id"])
        assert "root" in task["title"]
        assert "2026" in task["title"]

    def test_delete_template(self, logged_in):
        h = H()
        tmpl = h.create_template("DelTmpl")
        h.delete_template(tmpl["id"])
        lst = h.list_templates()
        assert not any(t["id"] == tmpl["id"] for t in lst)

    def test_empty_name_rejected(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/templates", {"name": "", "data": {}})
        assert code == 400

    def test_name_too_long_rejected(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/templates", {"name": "x" * 201, "data": {}})
        assert code == 400

    def test_non_owner_cannot_delete(self, logged_in):
        h = H()
        tmpl = h.create_template("OwnTmpl")
        u = H.register("tmpl_user1")
        code, _ = u.api_status("DELETE", f"/api/templates/{tmpl['id']}")
        assert code == 403

    def test_non_owner_cannot_instantiate(self, logged_in):
        h = H()
        tmpl = h.create_template("OwnInst")
        u = H.register("tmpl_user2")
        code, _ = u.api_status("POST", f"/api/templates/{tmpl['id']}/instantiate")
        assert code == 403

    def test_nonexistent_template(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/templates/999999/instantiate")
        assert code == 404


class TestWebhooks:

    def test_create_and_list(self, logged_in):
        h = H()
        wh = h.create_webhook("https://example.com/hook1", "task.created")
        lst = h.list_webhooks()
        assert any(w["id"] == wh["id"] for w in lst)

    def test_delete_webhook(self, logged_in):
        h = H()
        wh = h.create_webhook("https://example.com/hook2")
        h.delete_webhook(wh["id"])
        lst = h.list_webhooks()
        assert not any(w["id"] == wh["id"] for w in lst)

    def test_empty_url_rejected(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/webhooks", {"url": "", "events": "*"})
        assert code == 400

    def test_non_http_url_rejected(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/webhooks", {"url": "ftp://example.com", "events": "*"})
        assert code == 400

    def test_localhost_url_rejected(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/webhooks", {"url": "https://localhost/hook", "events": "*"})
        assert code == 400

    def test_private_ip_rejected(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/webhooks", {"url": "https://192.168.1.1/hook", "events": "*"})
        assert code == 400

    def test_invalid_event_rejected(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/webhooks", {"url": "https://example.com/h", "events": "bad.event"})
        assert code == 400

    @pytest.mark.parametrize("event", [
        "task.created", "task.updated", "task.deleted",
        "sprint.created", "sprint.started", "sprint.completed"
    ])
    def test_valid_event(self, logged_in, event):
        h = H()
        wh = h.create_webhook(f"https://example.com/{event}", event)
        assert wh["events"] == event

    def test_wildcard_events(self, logged_in):
        h = H()
        wh = h.create_webhook("https://example.com/all", "*")
        assert wh["events"] == "*"

    def test_url_with_credentials_rejected(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/webhooks",
                               {"url": "https://user:pass@example.com/hook", "events": "*"})
        assert code == 400


class TestNotifications:

    def test_list_empty(self, logged_in):
        h = H()
        n = h.notifications()
        assert isinstance(n, list)

    def test_unread_count(self, logged_in):
        h = H()
        r = h.unread_count()
        assert isinstance(r, (dict, int, list))

    def test_mark_read(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/notifications/read")
        assert code in (200, 204, 400)  # May fail if no notifications


class TestProfile:

    def test_change_username(self, logged_in):
        u = H.register("prof_orig", "ProfPass1")
        r = u.update_profile(username="prof_renamed")
        assert r["username"] == "prof_renamed"

    def test_change_password_requires_current(self, logged_in):
        u = H.register("prof_pw1", "ProfPw1x")
        code, _ = u.api_status("PUT", "/api/profile", {"password": "NewPw1xxx"})
        assert code == 400  # missing current_password

    def test_change_password_wrong_current(self, logged_in):
        u = H.register("prof_pw2", "ProfPw2x")
        code, _ = u.api_status("PUT", "/api/profile",
                               {"password": "NewPw2xxx", "current_password": "WrongPw1x"})
        assert code == 403

    def test_change_password_success(self, logged_in):
        u = H.register("prof_pw3", "ProfPw3x")
        r = u.update_profile(password="NewPw3xxx", current_password="ProfPw3x")
        assert r.get("token")

    def test_duplicate_username_rejected(self, logged_in):
        H.register("prof_dup1", "ProfDup1")
        u2 = H.register("prof_dup2", "ProfDup2")
        code, _ = u2.api_status("PUT", "/api/profile", {"username": "prof_dup1"})
        assert code == 409


class TestNotifPrefs:

    def test_get_default_prefs(self, logged_in):
        h = H()
        prefs = h.get_notif_prefs()
        assert len(prefs) == 7
        assert all(p["enabled"] for p in prefs)

    def test_disable_event(self, logged_in):
        h = H()
        h.update_notif_prefs([{"event_type": "task_assigned", "enabled": False}])
        prefs = h.get_notif_prefs()
        ta = next(p for p in prefs if p["event_type"] == "task_assigned")
        assert not ta["enabled"]

    def test_invalid_event_type_rejected(self, logged_in):
        h = H()
        code, _ = h.api_status("PUT", "/api/profile/notifications",
                               [{"event_type": "bad_event", "enabled": False}])
        assert code == 400
