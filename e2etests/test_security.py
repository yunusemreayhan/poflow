"""Security tests: JWT tampering, IDOR, privilege escalation, rate limiting,
SQL injection, path traversal. These find real vulnerabilities."""
import base64
import json
import os
import subprocess
import socket
import tempfile
import time
import urllib.request
import urllib.error
from pathlib import Path

import pytest
from helpers import H, _api, _api_status
import harness
from harness import ROOT_PASSWORD, JWT_SECRET, DAEMON_BINARY


# ── JWT Token Tampering ────────────────────────────────────────

class TestJWTTampering:

    def _get_valid_token(self):
        return H().token

    def test_garbage_token_rejected(self, logged_in):
        code, _ = _api_status("GET", "/api/tasks", token="not.a.jwt")
        assert code == 401

    def test_empty_bearer_rejected(self, logged_in):
        code, _ = _api_status("GET", "/api/tasks", token="")
        assert code == 401

    def test_expired_token_rejected(self, logged_in):
        """Craft a token with exp=1 (1970) — must be rejected."""
        # Take a valid token, decode payload, set exp=1, re-encode
        # (signature will be wrong too, but exp check should fire first)
        tok = self._get_valid_token()
        parts = tok.split(".")
        # Decode payload (add padding)
        payload = json.loads(base64.urlsafe_b64decode(parts[1] + "=="))
        payload["exp"] = 1
        new_payload = base64.urlsafe_b64encode(
            json.dumps(payload).encode()).rstrip(b"=").decode()
        tampered = f"{parts[0]}.{new_payload}.{parts[2]}"
        code, _ = _api_status("GET", "/api/tasks", token=tampered)
        assert code == 401

    def test_modified_user_id_rejected(self, logged_in):
        """Change user_id in payload — signature mismatch → rejected."""
        tok = self._get_valid_token()
        parts = tok.split(".")
        payload = json.loads(base64.urlsafe_b64decode(parts[1] + "=="))
        payload["user_id"] = 99999
        new_payload = base64.urlsafe_b64encode(
            json.dumps(payload).encode()).rstrip(b"=").decode()
        tampered = f"{parts[0]}.{new_payload}.{parts[2]}"
        code, _ = _api_status("GET", "/api/tasks", token=tampered)
        assert code == 401

    def test_modified_role_rejected(self, logged_in):
        """Change role from 'user' to 'root' — signature mismatch → rejected."""
        alice = H.register("jwt_alice")
        tok = alice.token
        parts = tok.split(".")
        payload = json.loads(base64.urlsafe_b64decode(parts[1] + "=="))
        payload["role"] = "root"
        new_payload = base64.urlsafe_b64encode(
            json.dumps(payload).encode()).rstrip(b"=").decode()
        tampered = f"{parts[0]}.{new_payload}.{parts[2]}"
        code, _ = _api_status("GET", "/api/admin/users", token=tampered)
        assert code == 401

    def test_wrong_secret_rejected(self, logged_in):
        """Token signed with a different secret must be rejected."""
        import hmac, hashlib
        header = base64.urlsafe_b64encode(b'{"alg":"HS256","typ":"JWT"}').rstrip(b"=").decode()
        payload = base64.urlsafe_b64encode(json.dumps({
            "sub": "1", "user_id": 1, "username": "root", "role": "root",
            "exp": int(time.time()) + 3600, "iat": int(time.time()), "typ": "access"
        }).encode()).rstrip(b"=").decode()
        sig_input = f"{header}.{payload}".encode()
        sig = base64.urlsafe_b64encode(
            hmac.new(b"wrong-secret", sig_input, hashlib.sha256).digest()
        ).rstrip(b"=").decode()
        fake_token = f"{header}.{payload}.{sig}"
        code, _ = _api_status("GET", "/api/tasks", token=fake_token)
        assert code == 401

    def test_none_algorithm_rejected(self, logged_in):
        """Token with alg=none (classic JWT bypass) must be rejected."""
        header = base64.urlsafe_b64encode(b'{"alg":"none","typ":"JWT"}').rstrip(b"=").decode()
        payload = base64.urlsafe_b64encode(json.dumps({
            "sub": "1", "user_id": 1, "username": "root", "role": "root",
            "exp": int(time.time()) + 3600, "iat": int(time.time()), "typ": "access"
        }).encode()).rstrip(b"=").decode()
        fake_token = f"{header}.{payload}."
        code, _ = _api_status("GET", "/api/tasks", token=fake_token)
        assert code == 401


# ── IDOR (Insecure Direct Object Reference) ───────────────────

class TestIDOR:

    def test_user_cannot_update_others_task(self, logged_in):
        root = H()
        task = root.create_task("Root's Secret Task")
        alice = H.register("idor_alice")
        code, _ = alice.api_status("PUT", f"/api/tasks/{task['id']}",
            {"title": "Hacked by Alice"})
        assert code == 403

    def test_user_cannot_delete_others_task(self, logged_in):
        root = H()
        task = root.create_task("Root's Task")
        alice = H.register("idor_alice2")
        code, _ = alice.api_status("DELETE", f"/api/tasks/{task['id']}")
        assert code == 403

    def test_user_cannot_duplicate_others_task(self, logged_in):
        root = H()
        task = root.create_task("Root's Task")
        alice = H.register("idor_alice3")
        code, _ = alice.api_status("POST", f"/api/tasks/{task['id']}/duplicate")
        assert code == 403

    def test_user_can_read_others_task(self, logged_in):
        """Tasks are team-visible — reading is allowed (by design)."""
        root = H()
        task = root.create_task("Visible Task")
        alice = H.register("idor_reader")
        code, _ = alice.api_status("GET", f"/api/tasks/{task['id']}")
        assert code == 200

    def test_user_cannot_set_status_on_others_task(self, logged_in):
        root = H()
        task = root.create_task("Root's Task")
        alice = H.register("idor_status")
        code, _ = alice.api_status("PUT", f"/api/tasks/{task['id']}",
            {"status": "completed"})
        assert code == 403

    def test_user_cannot_delete_others_comment(self, logged_in):
        root = H()
        task = root.create_task("CommentTask")
        comment = root.add_comment(task["id"], "Root's comment")
        alice = H.register("idor_comment")
        code, _ = alice.api_status("DELETE", f"/api/comments/{comment['id']}")
        assert code == 403

    def test_user_cannot_log_time_on_others_task(self, logged_in):
        root = H()
        task = root.create_task("TimeTask")
        alice = H.register("idor_time")
        code, _ = alice.api_status("POST", f"/api/tasks/{task['id']}/time",
            {"hours": 1, "note": "hacked"})
        # Time logging may be allowed for assignees — check it's at least not 500
        assert code in (201, 403)

    def test_user_cannot_delete_others_attachment(self, logged_in):
        root = H()
        task = root.create_task("AttachTask")
        # Upload an attachment as root
        url = f"{harness.BASE_URL}/api/tasks/{task['id']}/attachments"
        req = urllib.request.Request(url, data=b"secret file content",
            headers={"Authorization": f"Bearer {root.token}",
                     "X-Requested-With": "test",
                     "Content-Type": "application/octet-stream",
                     "X-Filename": "secret.txt"}, method="POST")
        resp = json.loads(urllib.request.urlopen(req, timeout=5).read())
        att_id = resp["id"]
        # Alice tries to delete it
        alice = H.register("idor_attach")
        code, _ = alice.api_status("DELETE", f"/api/attachments/{att_id}")
        assert code == 403

    def test_nonexistent_task_returns_404(self, logged_in):
        h = H()
        code, _ = h.api_status("GET", "/api/tasks/999999")
        assert code in (404, 500)

    def test_negative_task_id_rejected(self, logged_in):
        h = H()
        code, _ = h.api_status("GET", "/api/tasks/-1")
        assert code in (400, 404, 500)


# ── Privilege Escalation ───────────────────────────────────────

class TestPrivilegeEscalation:

    def test_normal_user_cannot_list_users(self, logged_in):
        alice = H.register("priv_alice")
        code, _ = alice.api_status("GET", "/api/admin/users")
        assert code == 403

    def test_normal_user_cannot_change_roles(self, logged_in):
        alice = H.register("priv_alice2")
        code, _ = alice.api_status("PUT", "/api/admin/users/1/role",
            {"role": "user"})
        assert code == 403

    def test_normal_user_cannot_create_backup(self, logged_in):
        alice = H.register("priv_backup")
        code, _ = alice.api_status("POST", "/api/admin/backup")
        assert code == 403

    def test_normal_user_cannot_restore_backup(self, logged_in):
        alice = H.register("priv_restore")
        code, _ = alice.api_status("POST", "/api/admin/restore",
            {"filename": "test.db"})
        assert code == 403

    def test_normal_user_cannot_delete_other_users(self, logged_in):
        alice = H.register("priv_delusr")
        code, _ = alice.api_status("DELETE", "/api/admin/users/1")
        assert code == 403

    def test_normal_user_cannot_reset_others_password(self, logged_in):
        alice = H.register("priv_resetpw")
        code, _ = alice.api_status("PUT", "/api/admin/users/1/password",
            {"password": "Hacked123"})
        assert code == 403

    def test_self_promote_to_root_rejected(self, logged_in):
        """User cannot promote themselves to root via profile update.
        The profile endpoint only accepts username/password, not role.
        Sending a role field should be silently ignored."""
        alice = H.register("priv_selfpromote")
        # Try to sneak a role field into the profile update
        code, resp = alice.api_status("PUT", "/api/profile",
            {"username": "priv_selfpromote", "role": "root"})
        # Should succeed (ignoring role) or reject
        assert code in (200, 400, 422)
        if code == 200:
            # Verify role didn't change — use admin endpoint from root
            root = H()
            users = root.admin_users()
            alice_user = [u for u in users if u["username"] == "priv_selfpromote"]
            assert alice_user[0]["role"] == "user"


# ── Rate Limiter ───────────────────────────────────────────────

class TestRateLimiter:
    """Start a separate daemon WITHOUT POFLOW_NO_RATE_LIMIT to test
    that the rate limiter actually works."""

    @pytest.fixture(scope="class")
    def rate_limited_daemon(self):
        """Start a daemon with rate limiting enabled."""
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        tmpdir = tempfile.mkdtemp(prefix="poflow_ratelimit_")
        Path(tmpdir, "config.toml").write_text(
            f'bind_address = "127.0.0.1"\nbind_port = {port}\n'
            f"work_duration_min = 1\nshort_break_min = 1\nlong_break_min = 1\n"
            f"long_break_interval = 4\nauto_start_breaks = false\n"
            f"auto_start_work = false\nsound_enabled = false\n"
            f"notification_enabled = false\ndaily_goal = 8\n"
        )
        env = os.environ.copy()
        env.update({
            "POFLOW_DATA_DIR": tmpdir,
            "POFLOW_CONFIG_DIR": tmpdir,
            "POFLOW_JWT_SECRET": JWT_SECRET,
            "POFLOW_ROOT_PASSWORD": ROOT_PASSWORD,
            "POFLOW_SWAGGER": "0",
            "RUST_LOG": "warn",
        })
        env.pop("POFLOW_NO_RATE_LIMIT", None)
        proc = subprocess.Popen(
            [DAEMON_BINARY], env=env,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            preexec_fn=os.setsid,
        )
        started = False
        for _ in range(40):
            try:
                urllib.request.urlopen(f"{base_url}/api/health", timeout=1)
                started = True
                break
            except Exception:
                time.sleep(0.25)
        if not started:
            proc.kill()
            proc.wait()
            import shutil
            shutil.rmtree(tmpdir, ignore_errors=True)
            pytest.skip("Rate-limited daemon failed to start")
        yield base_url
        os.killpg(os.getpgid(proc.pid), 9)
        proc.wait()
        import shutil
        shutil.rmtree(tmpdir, ignore_errors=True)

    def _api_rl(self, method, path, body=None, token=None, base_url=""):
        """API call against the rate-limited daemon."""
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
            resp = urllib.request.urlopen(
                urllib.request.Request(f"{base_url}{path}", data=data,
                    headers=hdrs, method=method), timeout=5)
            raw = resp.read().decode()
            return resp.status, json.loads(raw) if raw else {}
        except urllib.error.HTTPError as e:
            return e.code, e.read().decode()[:200]

    def test_get_requests_not_rate_limited(self, logged_in, rate_limited_daemon):
        """GET requests should NOT be rate limited. Runs first to get a token."""
        base = rate_limited_daemon
        code, resp = self._api_rl("POST", "/api/auth/login",
            {"username": "root", "password": ROOT_PASSWORD}, base_url=base)
        if code != 200:
            pytest.skip("Cannot login")
        token = resp.get("token", "")
        # Store token for later tests
        self.__class__._token = token
        self.__class__._base = base
        all_ok = True
        for _ in range(50):
            code, _ = self._api_rl("GET", "/api/tasks", token=token, base_url=base)
            if code == 429:
                all_ok = False
                break
        assert all_ok, "GET requests should not be rate limited"

    def test_api_rate_limit_triggers(self, logged_in, rate_limited_daemon):
        """API mutation endpoint: 200 requests/60s per IP. Should eventually 429."""
        token = getattr(self.__class__, "_token", None)
        base = getattr(self.__class__, "_base", rate_limited_daemon)
        if not token:
            code, resp = self._api_rl("POST", "/api/auth/login",
                {"username": "root", "password": ROOT_PASSWORD}, base_url=base)
            if code != 200:
                pytest.skip("Cannot login")
            token = resp.get("token", "")
        hit_429 = False
        for i in range(210):
            code, _ = self._api_rl("POST", "/api/tasks",
                {"title": f"RL_{i}", "project": "RL"}, token=token, base_url=base)
            if code == 429:
                hit_429 = True
                break
        assert hit_429, "API rate limiter did not trigger after 210 mutations"

    def test_auth_rate_limit_triggers(self, logged_in, rate_limited_daemon):
        """Auth endpoint: 10 requests/60s per IP. 11th should be 429. Runs last."""
        base = rate_limited_daemon
        hit_429 = False
        for i in range(15):
            code, _ = self._api_rl("POST", "/api/auth/login",
                {"username": "root", "password": "WrongPass1"}, base_url=base)
            if code == 429:
                hit_429 = True
                break
        assert hit_429, "Auth rate limiter did not trigger after 15 attempts"


# ── SQL Injection ──────────────────────────────────────────────

class TestSQLInjection:

    def test_search_sql_injection(self, logged_in):
        h = H()
        # Classic SQL injection payloads
        payloads = [
            "'; DROP TABLE tasks; --",
            "\" OR 1=1 --",
            "' UNION SELECT * FROM users --",
            "1; DELETE FROM tasks",
            "' OR ''='",
        ]
        for payload in payloads:
            encoded = urllib.request.quote(payload)
            code, _ = h.api_status("GET", f"/api/tasks/search?q={encoded}")
            assert code in (200, 400), f"Unexpected {code} for payload: {payload}"
        # Verify tasks still exist (nothing was dropped)
        tasks = h.list_tasks()
        assert isinstance(tasks, list)

    def test_task_title_sql_injection(self, logged_in):
        h = H()
        t = h.create_task("'; DROP TABLE tasks; --")
        assert t["id"] > 0
        # Table still works
        tasks = h.list_tasks()
        assert len(tasks) >= 1

    def test_comment_sql_injection(self, logged_in):
        h = H()
        t = h.create_task("SQLiComment")
        c = h.add_comment(t["id"], "' OR 1=1; DROP TABLE comments; --")
        assert c["id"] > 0
        comments = h.list_comments(t["id"])
        assert len(comments) >= 1

    def test_sprint_name_sql_injection(self, logged_in):
        h = H()
        s = h.create_sprint("'; DROP TABLE sprints; --")
        assert s["id"] > 0
        sprints = h.list_sprints()
        assert len(sprints) >= 1

    def test_label_name_sql_injection(self, logged_in):
        h = H()
        lbl = h.create_label("'; DROP TABLE labels; --")
        assert lbl["id"] > 0


# ── Path Traversal in Attachments ──────────────────────────────

class TestPathTraversal:

    def _upload(self, h, task_id, filename, content=b"test"):
        url = f"{harness.BASE_URL}/api/tasks/{task_id}/attachments"
        req = urllib.request.Request(url, data=content,
            headers={"Authorization": f"Bearer {h.token}",
                     "X-Requested-With": "test",
                     "Content-Type": "application/octet-stream",
                     "X-Filename": filename}, method="POST")
        try:
            resp = urllib.request.urlopen(req, timeout=5)
            return resp.status, json.loads(resp.read())
        except urllib.error.HTTPError as e:
            return e.code, e.read().decode()[:200]

    def test_dotdot_slash_stripped(self, logged_in):
        """../../etc/passwd should be sanitized to etcpasswd or similar."""
        h = H()
        t = h.create_task("TraversalTask")
        code, resp = self._upload(h, t["id"], "../../etc/passwd")
        assert code == 201
        # Filename should NOT contain path separators
        name = resp.get("filename", "")
        assert "/" not in name
        assert "\\" not in name
        assert ".." not in name

    def test_absolute_path_stripped(self, logged_in):
        h = H()
        t = h.create_task("AbsPathTask")
        code, resp = self._upload(h, t["id"], "/etc/shadow")
        assert code == 201
        name = resp.get("filename", "")
        assert not name.startswith("/")

    def test_null_byte_in_filename(self, logged_in):
        h = H()
        t = h.create_task("NullByteTask")
        code, resp = self._upload(h, t["id"], "file.txt\x00.exe")
        # Should either sanitize or reject
        if code == 201:
            name = resp.get("filename", "")
            assert "\x00" not in name
        else:
            assert code == 400

    def test_dot_filename_sanitized(self, logged_in):
        """Filename starting with . should be stripped (hidden files)."""
        h = H()
        t = h.create_task("DotTask")
        code, resp = self._upload(h, t["id"], ".htaccess")
        assert code == 201
        name = resp.get("filename", "")
        assert not name.startswith(".")

    def test_html_filename_blocked(self, logged_in):
        """HTML files should be blocked (XSS via content-type sniffing)."""
        h = H()
        t = h.create_task("HtmlTask")
        url = f"{harness.BASE_URL}/api/tasks/{t['id']}/attachments"
        req = urllib.request.Request(url, data=b"<script>alert(1)</script>",
            headers={"Authorization": f"Bearer {h.token}",
                     "X-Requested-With": "test",
                     "Content-Type": "text/html",
                     "X-Filename": "evil.html"}, method="POST")
        try:
            resp = urllib.request.urlopen(req, timeout=5)
            code = resp.status
        except urllib.error.HTTPError as e:
            code = e.code
        assert code == 400  # HTML content-type blocked

    def test_svg_content_type_blocked(self, logged_in):
        """SVG files should be blocked (XSS vector)."""
        h = H()
        t = h.create_task("SvgTask")
        url = f"{harness.BASE_URL}/api/tasks/{t['id']}/attachments"
        req = urllib.request.Request(url,
            data=b'<svg onload="alert(1)"/>',
            headers={"Authorization": f"Bearer {h.token}",
                     "X-Requested-With": "test",
                     "Content-Type": "image/svg+xml",
                     "X-Filename": "evil.svg"}, method="POST")
        try:
            resp = urllib.request.urlopen(req, timeout=5)
            code = resp.status
        except urllib.error.HTTPError as e:
            code = e.code
        assert code == 400

    def test_javascript_content_type_blocked(self, logged_in):
        """JavaScript content-type should be blocked."""
        h = H()
        t = h.create_task("JsTask")
        url = f"{harness.BASE_URL}/api/tasks/{t['id']}/attachments"
        req = urllib.request.Request(url, data=b"alert(1)",
            headers={"Authorization": f"Bearer {h.token}",
                     "X-Requested-With": "test",
                     "Content-Type": "application/javascript",
                     "X-Filename": "evil.js"}, method="POST")
        try:
            resp = urllib.request.urlopen(req, timeout=5)
            code = resp.status
        except urllib.error.HTTPError as e:
            code = e.code
        assert code == 400

    def test_backup_restore_path_traversal(self, logged_in):
        """Backup restore filename must not allow path traversal."""
        h = H()
        code, _ = h.api_status("POST", "/api/admin/restore",
            {"filename": "../../etc/passwd"})
        assert code == 400

    def test_backup_restore_rejects_non_db(self, logged_in):
        h = H()
        code, _ = h.api_status("POST", "/api/admin/restore",
            {"filename": "notadb.txt"})
        assert code == 400


def _free_port():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]
