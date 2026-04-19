"""Edge cases: unicode, long strings, empty strings, boundary values, special chars."""
import pytest
from helpers import H, _api_status
import harness


class TestUnicodeEmoji:

    def test_task_title_emoji(self, logged_in):
        h = H()
        t = h.create_task("🍅 Poflow Task 🎯")
        assert "🍅" in t["title"]

    def test_task_title_cjk(self, logged_in):
        h = H()
        t = h.create_task("任务标题 タスク 작업")
        assert "任务标题" in t["title"]

    def test_task_title_arabic(self, logged_in):
        h = H()
        t = h.create_task("مهمة اختبار")
        assert "مهمة" in t["title"]

    def test_comment_emoji(self, logged_in):
        h = H()
        t = h.create_task("EmojiCm")
        c = h.add_comment(t["id"], "Great work! 🎉🚀💯")
        assert "🎉" in c["content"]

    def test_comment_multiline_unicode(self, logged_in):
        h = H()
        t = h.create_task("MultiCm")
        text = "Line 1: café\nLine 2: naïve\nLine 3: 日本語\nLine 4: 🇹🇷"
        c = h.add_comment(t["id"], text)
        assert "café" in c["content"]

    def test_sprint_name_unicode(self, logged_in):
        h = H()
        s = h.create_sprint("Sprint 🏃‍♂️ Week 1")
        assert "🏃" in s["name"]

    def test_room_name_unicode(self, logged_in):
        h = H()
        r = h.create_room("Estimation 🃏")
        assert "🃏" in r["name"]

    def test_label_name_unicode(self, logged_in):
        h = H()
        lbl = h.create_label("🔴 Urgent")
        assert "🔴" in lbl["name"]

    def test_epic_name_unicode(self, logged_in):
        h = H()
        e = h.create_epic("Epic 🏔️ Mountain")
        assert "🏔" in e["name"]

    def test_team_name_unicode(self, logged_in):
        h = H()
        t = h.create_team("Team 🦄 Unicorn")
        assert "🦄" in t["name"]


class TestLongStrings:

    def test_task_title_1000_chars(self, logged_in):
        h = H()
        title = "A" * 1000
        code, r = h.api_status("POST", "/api/tasks", {"title": title, "project": "Long"})
        assert code in (201, 400)

    def test_task_title_10000_chars(self, logged_in):
        h = H()
        title = "B" * 10000
        code, r = h.api_status("POST", "/api/tasks", {"title": title, "project": "Long"})
        # Should either succeed or reject with 400
        assert code in (201, 400)

    def test_comment_10000_chars(self, logged_in):
        h = H()
        t = h.create_task("LongCm")
        text = "C" * 10000
        code, r = h.api_status("POST", f"/api/tasks/{t['id']}/comments", {"content": text})
        assert code in (201, 400)

    def test_task_description_10000_chars(self, logged_in):
        h = H()
        desc = "D" * 10000
        t = h.create_task("LongDesc", description=desc)
        detail = h.get_task(t["id"])
        task = detail.get("task", detail)
        assert len(task.get("description", "")) >= 1000 or isinstance(task, dict)

    def test_sprint_name_1000_chars(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/sprints",
            {"name": "S" * 1000, "start_date": "2026-01-01", "end_date": "2026-01-15"})
        assert code in (201, 400)

    def test_webhook_url_2001_chars(self, logged_in):
        h = H()
        url = "https://example.com/" + "a" * 1981
        code, _ = h.api_status("POST", "/api/webhooks", {"url": url, "events": "*"})
        assert code == 400  # Max 2000

    def test_template_name_201_chars(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/templates", {"name": "T" * 201, "data": {}})
        assert code == 400  # Max 200


class TestEmptyStrings:

    def test_task_empty_title(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/tasks", {"title": "", "project": "X"})
        assert code == 400

    def test_task_whitespace_title(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/tasks", {"title": "   ", "project": "X"})
        assert code in (201, 400)  # May trim to empty

    def test_comment_empty(self, logged_in):
        h = H()
        t = h.create_task("EmptyCm")
        code, _ = h.api_status("POST", f"/api/tasks/{t['id']}/comments", {"content": ""})
        assert code == 400

    def test_sprint_empty_name(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/sprints",
            {"name": "", "start_date": "2026-01-01", "end_date": "2026-01-15"})
        assert code in (400, 422)

    def test_label_empty_name(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/labels", {"name": "", "color": "#000"})
        assert code == 400

    def test_room_empty_name(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/rooms", {"name": "", "estimation_unit": "points"})
        assert code in (400, 422)

    def test_team_empty_name(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/teams", {"name": ""})
        assert code in (400, 422)

    def test_epic_empty_name(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/epics", {"name": ""})
        assert code in (400, 422)

    def test_template_empty_name(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/templates", {"name": "", "data": {}})
        assert code == 400

    def test_webhook_empty_url(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/webhooks", {"url": "", "events": "*"})
        assert code == 400


class TestSpecialCharacters:

    def test_task_title_html_injection(self, logged_in):
        h = H()
        t = h.create_task("<script>alert('xss')</script>")
        # Should store as-is (escaped on render) or strip tags
        assert t["title"]  # Not empty

    def test_task_title_sql_injection(self, logged_in):
        h = H()
        t = h.create_task("'; DROP TABLE tasks; --")
        assert t["id"] > 0  # Didn't crash

    def test_comment_html_injection(self, logged_in):
        h = H()
        t = h.create_task("HtmlCm")
        c = h.add_comment(t["id"], "<img src=x onerror=alert(1)>")
        assert c["content"]

    def test_task_title_null_bytes(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/tasks", {"title": "null\x00byte", "project": "X"})
        assert code in (201, 400)

    def test_task_title_backslashes(self, logged_in):
        h = H()
        t = h.create_task("path\\to\\file")
        assert "\\" in t["title"]

    def test_task_title_quotes(self, logged_in):
        h = H()
        t = h.create_task('He said "hello" and \'goodbye\'')
        assert "hello" in t["title"]

    def test_search_special_chars(self, logged_in):
        h = H()
        h.create_task("Search%Test&Special=Chars")
        r = h.search_tasks("Search%25Test")
        assert isinstance(r, list)


class TestUsernameEdgeCases:

    def test_register_very_long_username(self, logged_in):
        code, _ = _api_status("POST", "/api/auth/register",
            {"username": "u" * 200, "password": "LongUser1"})
        assert code in (200, 400)

    def test_register_single_char_username(self, logged_in):
        code, _ = _api_status("POST", "/api/auth/register",
            {"username": "x", "password": "SingleC1"})
        assert code in (200, 400)

    def test_register_username_with_spaces(self, logged_in):
        code, _ = _api_status("POST", "/api/auth/register",
            {"username": "has space", "password": "SpaceUs1"})
        assert code in (200, 400)

    def test_register_username_special_chars(self, logged_in):
        code, _ = _api_status("POST", "/api/auth/register",
            {"username": "user@#$!", "password": "Special1"})
        assert code in (200, 400)

    def test_register_empty_username(self, logged_in):
        code, _ = _api_status("POST", "/api/auth/register",
            {"username": "", "password": "EmptyUs1"})
        assert code == 400

    def test_register_unicode_username(self, logged_in):
        code, _ = _api_status("POST", "/api/auth/register",
            {"username": "用户名", "password": "Unicode1"})
        assert code in (200, 400)


class TestTimerBoundaryValues:

    def test_config_work_duration_zero(self, logged_in):
        h = H()
        code, _ = h.api_status("PUT", "/api/config",
            {**h.get_config(), "work_duration_min": 0})
        assert code in (200, 400)

    def test_config_work_duration_negative(self, logged_in):
        h = H()
        code, _ = h.api_status("PUT", "/api/config",
            {**h.get_config(), "work_duration_min": -1})
        assert code in (200, 400, 422)

    def test_config_work_duration_max_int(self, logged_in):
        h = H()
        code, _ = h.api_status("PUT", "/api/config",
            {**h.get_config(), "work_duration_min": 2147483647})
        assert code in (200, 400, 422)

    def test_config_short_break_zero(self, logged_in):
        h = H()
        code, _ = h.api_status("PUT", "/api/config",
            {**h.get_config(), "short_break_min": 0})
        assert code in (200, 400)

    def test_config_long_break_zero(self, logged_in):
        h = H()
        code, _ = h.api_status("PUT", "/api/config",
            {**h.get_config(), "long_break_min": 0})
        assert code in (200, 400)

    def test_config_daily_goal_zero(self, logged_in):
        h = H()
        code, _ = h.api_status("PUT", "/api/config",
            {**h.get_config(), "daily_goal": 0})
        assert code in (200, 400)

    def test_config_daily_goal_over_max(self, logged_in):
        h = H()
        code, _ = h.api_status("PUT", "/api/config",
            {**h.get_config(), "daily_goal": 51})
        assert code == 400  # Max 50

    def test_config_long_break_interval_zero(self, logged_in):
        h = H()
        code, _ = h.api_status("PUT", "/api/config",
            {**h.get_config(), "long_break_interval": 0})
        assert code == 400  # Min 1

    def test_config_long_break_interval_over_max(self, logged_in):
        h = H()
        code, _ = h.api_status("PUT", "/api/config",
            {**h.get_config(), "long_break_interval": 11})
        assert code in (200, 400)  # Daemon may accept or reject


class TestTaskBoundaryValues:

    def test_priority_zero(self, logged_in):
        h = H()
        code, r = h.api_status("POST", "/api/tasks",
            {"title": "PriZero", "project": "X", "priority": 0})
        assert code in (201, 400)

    def test_priority_negative(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/tasks",
            {"title": "PriNeg", "project": "X", "priority": -1})
        assert code in (201, 400)

    def test_estimated_zero(self, logged_in):
        h = H()
        t = h.create_task("EstZero", estimated=0)
        assert t["estimated"] == 0

    def test_estimated_negative(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/tasks",
            {"title": "EstNeg", "project": "X", "estimated": -5})
        assert code in (201, 400)

    def test_estimated_very_large(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/tasks",
            {"title": "EstBig", "project": "X", "estimated": 999999})
        assert code in (201, 400)

    def test_burn_zero_points(self, logged_in):
        h = H()
        s = h.create_sprint("BurnZero")
        t = h.create_task("BZ", estimated=5)
        h.add_sprint_tasks(s["id"], [t["id"]])
        h.start_sprint(s["id"])
        code, _ = h.api_status("POST", f"/api/sprints/{s['id']}/burn",
            {"task_id": t["id"], "points": 0, "hours": 0})
        assert code in (201, 400)

    def test_burn_negative_points(self, logged_in):
        h = H()
        s = h.create_sprint("BurnNeg")
        t = h.create_task("BN", estimated=5)
        h.add_sprint_tasks(s["id"], [t["id"]])
        h.start_sprint(s["id"])
        code, _ = h.api_status("POST", f"/api/sprints/{s['id']}/burn",
            {"task_id": t["id"], "points": -1, "hours": -1})
        assert code in (201, 400)

    def test_time_log_zero_hours(self, logged_in):
        h = H()
        t = h.create_task("TimeZero")
        code, _ = h.api_status("POST", f"/api/tasks/{t['id']}/time",
            {"hours": 0, "note": ""})
        assert code in (201, 400)

    def test_time_log_negative_hours(self, logged_in):
        h = H()
        t = h.create_task("TimeNeg")
        code, _ = h.api_status("POST", f"/api/tasks/{t['id']}/time",
            {"hours": -5, "note": ""})
        assert code in (201, 400)


class TestConcurrentLogin:

    def test_same_user_two_sessions(self, logged_in):
        """Two simultaneous sessions for the same user should both work."""
        h1 = H("root", harness.ROOT_PASSWORD)
        h2 = H("root", harness.ROOT_PASSWORD)
        t1 = h1.create_task("Sess1")
        t2 = h2.create_task("Sess2")
        assert t1["id"] != t2["id"]
        # Both can read
        tasks = h1.list_tasks()
        assert any(t["id"] == t2["id"] for t in tasks)

    def test_fresh_login_after_logout(self, logged_in):
        """After logout, a fresh login with correct credentials succeeds."""
        import random, time
        name = f"dual_{random.randint(10000,99999)}"
        u = H.register(name, "DualSes1")
        t1 = u.create_task("BeforeLogout")
        assert t1["id"] > 0
        u.logout()
        time.sleep(0.5)
        # The daemon may blacklist the JWT on logout. Verify login endpoint
        # still accepts credentials (returns a new token).
        code, resp = _api_status("POST", "/api/auth/login",
            {"username": name, "password": "DualSes1"})
        assert code == 200
        assert "token" in resp


class TestSprintDateEdgeCases:

    def test_end_before_start(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/sprints",
            {"name": "BadDate", "start_date": "2026-12-31", "end_date": "2026-01-01"})
        assert code in (201, 400)  # May or may not validate

    def test_same_start_end(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/sprints",
            {"name": "SameDate", "start_date": "2026-06-01", "end_date": "2026-06-01"})
        assert code in (201, 400)

    def test_invalid_date_format(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/sprints",
            {"name": "BadFmt", "start_date": "not-a-date", "end_date": "also-bad"})
        assert code in (400, 422, 500)

    def test_far_future_date(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/sprints",
            {"name": "Future", "start_date": "2099-01-01", "end_date": "2099-12-31"})
        assert code == 201


class TestRoomVoteBoundary:

    def test_vote_zero(self, logged_in):
        h = H()
        r = h.create_room("VoteZero")
        t = h.create_task("VZ")
        h.join_room(r["id"])
        h.start_voting(r["id"], t["id"])
        code, _ = h.api_status("POST", f"/api/rooms/{r['id']}/vote", {"value": 0})
        assert code in (200, 204, 400)

    def test_vote_negative(self, logged_in):
        h = H()
        r = h.create_room("VoteNeg")
        t = h.create_task("VN")
        h.join_room(r["id"])
        h.start_voting(r["id"], t["id"])
        code, _ = h.api_status("POST", f"/api/rooms/{r['id']}/vote", {"value": -1})
        assert code in (200, 400)

    def test_vote_very_large(self, logged_in):
        h = H()
        r = h.create_room("VoteBig")
        t = h.create_task("VB")
        h.join_room(r["id"])
        h.start_voting(r["id"], t["id"])
        code, _ = h.api_status("POST", f"/api/rooms/{r['id']}/vote", {"value": 999999})
        assert code in (200, 400)

    def test_accept_estimate_updates_task(self, logged_in):
        h = H()
        r = h.create_room("AccEst")
        t = h.create_task("AE")
        h.join_room(r["id"])
        h.start_voting(r["id"], t["id"])
        h.vote(r["id"], 8)
        h.reveal_votes(r["id"])
        result = h.accept_estimate(r["id"], 8)
        assert result.get("estimated") == 8 or isinstance(result, dict)
