"""Test harness: spins up an isolated pomodoro-daemon per test session.

Each session gets a fresh temp directory with its own DB, config, and JWT secret.
The daemon is killed and the temp dir cleaned up after all tests complete.
"""

from __future__ import annotations

import json
import os
import signal
import subprocess
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

import urllib.request
import urllib.error

DAEMON_BINARY = str(
    Path(__file__).resolve().parent.parent / "target" / "release" / "pomodoro-daemon"
)
TEST_PORT = 19090
BASE_URL = f"http://127.0.0.1:{TEST_PORT}"
ROOT_PASSWORD = "TestRoot1"
JWT_SECRET = "test-secret-for-flow-tests-1234567890abcdef"


class ApiError(Exception):
    def __init__(self, status: int, body: str):
        self.status = status
        self.body = body
        super().__init__(f"HTTP {status}: {body}")


@dataclass
class Client:
    """Thin HTTP client for the pomodoro API."""

    base: str = BASE_URL
    token: Optional[str] = None
    refresh_token: Optional[str] = None
    username: Optional[str] = None
    role: Optional[str] = None
    user_id: Optional[int] = None

    def request(
        self, method: str, path: str, body: dict = None, headers: dict = None
    ) -> dict | list | str:
        url = f"{self.base}{path}"
        data = json.dumps(body).encode() if body is not None else None
        hdrs = {"Content-Type": "application/json", "X-Requested-With": "test"}
        if self.token:
            hdrs["Authorization"] = f"Bearer {self.token}"
        if headers:
            hdrs.update(headers)
        req = urllib.request.Request(url, data=data, headers=hdrs, method=method)
        try:
            resp = urllib.request.urlopen(req, timeout=10)
            raw = resp.read().decode()
            if not raw:
                return {}
            return json.loads(raw)
        except urllib.error.HTTPError as e:
            raise ApiError(e.code, e.read().decode()) from e

    def get(self, path: str) -> dict | list:
        return self.request("GET", path)

    def post(self, path: str, body: dict = None) -> dict | list:
        return self.request("POST", path, body)

    def put(self, path: str, body: dict = None) -> dict | list:
        return self.request("PUT", path, body)

    def delete(self, path: str) -> dict | list:
        return self.request("DELETE", path)

    # ── Auth helpers ────────────────────────────────────────────

    def login(self, username: str, password: str) -> dict:
        resp = self.post("/api/auth/login", {"username": username, "password": password})
        self.token = resp["token"]
        self.refresh_token = resp["refresh_token"]
        self.username = resp["username"]
        self.role = resp["role"]
        self.user_id = resp["user_id"]
        return resp

    def register(self, username: str, password: str) -> dict:
        resp = self.post("/api/auth/register", {"username": username, "password": password})
        self.token = resp["token"]
        self.refresh_token = resp["refresh_token"]
        self.username = resp["username"]
        self.role = resp["role"]
        self.user_id = resp["user_id"]
        return resp

    def login_root(self) -> dict:
        return self.login("root", ROOT_PASSWORD)

    def as_new_user(self, username: str, password: str = "TestPass1") -> "Client":
        """Register a new user and return a Client logged in as them."""
        c = Client(base=self.base)
        c.register(username, password)
        return c

    def logout(self) -> None:
        self.post("/api/auth/logout")
        self.token = None
        self.refresh_token = None


@dataclass
class Daemon:
    """Manages an isolated pomodoro-daemon process."""

    proc: Optional[subprocess.Popen] = None
    tmpdir: Optional[str] = None

    def start(self) -> None:
        self.tmpdir = tempfile.mkdtemp(prefix="pomodoro_test_")
        # Write config with test port
        config = Path(self.tmpdir) / "config.toml"
        config.write_text(
            f"bind_address = \"127.0.0.1\"\n"
            f"bind_port = {TEST_PORT}\n"
            f"work_duration_min = 25\n"
            f"short_break_min = 5\n"
            f"long_break_min = 15\n"
            f"long_break_interval = 4\n"
            f"auto_start_breaks = false\n"
            f"auto_start_work = false\n"
            f"sound_enabled = false\n"
            f"notification_enabled = false\n"
            f"daily_goal = 8\n"
            f"auto_archive_days = 90\n"
        )
        env = os.environ.copy()
        env["POMODORO_DATA_DIR"] = self.tmpdir
        env["POMODORO_CONFIG_DIR"] = self.tmpdir
        env["POMODORO_JWT_SECRET"] = JWT_SECRET
        env["POMODORO_ROOT_PASSWORD"] = ROOT_PASSWORD
        env["POMODORO_SWAGGER"] = "0"
        env["RUST_LOG"] = "warn"

        self.proc = subprocess.Popen(
            [DAEMON_BINARY],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            preexec_fn=os.setsid,
        )
        # Wait for it to be ready
        for _ in range(40):
            try:
                urllib.request.urlopen(f"{BASE_URL}/api/health", timeout=1)
                return
            except Exception:
                time.sleep(0.25)
        raise RuntimeError("Daemon failed to start")

    def stop(self) -> None:
        if self.proc:
            os.killpg(os.getpgid(self.proc.pid), signal.SIGTERM)
            self.proc.wait(timeout=5)
            self.proc = None
        if self.tmpdir:
            import shutil
            shutil.rmtree(self.tmpdir, ignore_errors=True)
            self.tmpdir = None
