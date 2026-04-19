"""Performance tests: latency, startup time, memory, throughput.

Catches performance regressions early with hard thresholds.
Uses a dedicated daemon (no GUI) for accurate measurements.
"""
import os
import socket
import subprocess
import tempfile
import time
import json
import urllib.request
import urllib.error
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

import pytest
from harness import DAEMON_BINARY, JWT_SECRET, ROOT_PASSWORD


def _free_port():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def _api(method, path, body=None, token=None, base_url=""):
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
                headers=hdrs, method=method), timeout=10)
        raw = resp.read().decode()
        return resp.status, json.loads(raw) if raw else {}
    except urllib.error.HTTPError as e:
        return e.code, {}


def _timed_api(method, path, body=None, token=None, base_url=""):
    """Returns (status, response, elapsed_ms)."""
    t0 = time.perf_counter()
    code, resp = _api(method, path, body, token, base_url)
    elapsed = (time.perf_counter() - t0) * 1000
    return code, resp, elapsed


@pytest.fixture(scope="module")
def perf_daemon():
    """Dedicated daemon for performance tests — no GUI, no rate limit."""
    port = _free_port()
    base_url = f"http://127.0.0.1:{port}"
    tmpdir = tempfile.mkdtemp(prefix="poflow_perf_")
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
        "POFLOW_NO_RATE_LIMIT": "1",
        "RUST_LOG": "warn",
    })
    proc = subprocess.Popen(
        [DAEMON_BINARY], env=env,
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        preexec_fn=os.setsid,
    )
    for _ in range(40):
        try:
            urllib.request.urlopen(f"{base_url}/api/health", timeout=1)
            break
        except Exception:
            time.sleep(0.25)
    else:
        proc.kill()
        proc.wait()
        pytest.skip("Perf daemon failed to start")

    # Login and get token
    _, resp = _api("POST", "/api/auth/login",
        {"username": "root", "password": ROOT_PASSWORD}, base_url=base_url)
    token = resp["token"]

    yield {"base_url": base_url, "token": token, "proc": proc, "tmpdir": tmpdir}

    os.killpg(os.getpgid(proc.pid), 9)
    proc.wait()
    import shutil
    shutil.rmtree(tmpdir, ignore_errors=True)


# ── Startup Time ───────────────────────────────────────────────

class TestStartupTime:

    def test_daemon_starts_within_3_seconds(self):
        """Cold start: daemon should pass health check within 3s."""
        port = _free_port()
        tmpdir = tempfile.mkdtemp(prefix="poflow_startup_")
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
            "POFLOW_NO_RATE_LIMIT": "1",
            "RUST_LOG": "warn",
        })
        t0 = time.perf_counter()
        proc = subprocess.Popen(
            [DAEMON_BINARY], env=env,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            preexec_fn=os.setsid,
        )
        ready = False
        while time.perf_counter() - t0 < 3.0:
            try:
                urllib.request.urlopen(
                    f"http://127.0.0.1:{port}/api/health", timeout=0.5)
                ready = True
                break
            except Exception:
                time.sleep(0.05)
        elapsed = time.perf_counter() - t0
        os.killpg(os.getpgid(proc.pid), 9)
        proc.wait()
        import shutil
        shutil.rmtree(tmpdir, ignore_errors=True)
        assert ready, f"Daemon not ready after {elapsed:.1f}s (limit: 3s)"
        assert elapsed < 3.0, f"Startup took {elapsed:.1f}s (limit: 3s)"


# ── API Latency (empty DB) ────────────────────────────────────

class TestLatencyEmptyDB:

    def test_health_under_100ms(self, perf_daemon):
        d = perf_daemon
        _, _, ms = _timed_api("GET", "/api/health", base_url=d["base_url"])
        assert ms < 100, f"Health: {ms:.0f}ms"

    def test_login_under_500ms(self, perf_daemon):
        """Login includes bcrypt(cost=12) verification — allow 500ms under load."""
        d = perf_daemon
        _, _, ms = _timed_api("POST", "/api/auth/login",
            {"username": "root", "password": ROOT_PASSWORD}, base_url=d["base_url"])
        assert ms < 500, f"Login: {ms:.0f}ms"

    def test_list_tasks_under_100ms(self, perf_daemon):
        d = perf_daemon
        _, _, ms = _timed_api("GET", "/api/tasks", token=d["token"],
            base_url=d["base_url"])
        assert ms < 100, f"List tasks: {ms:.0f}ms"

    def test_create_task_under_100ms(self, perf_daemon):
        d = perf_daemon
        _, _, ms = _timed_api("POST", "/api/tasks",
            {"title": "PerfTask", "project": "Perf"}, token=d["token"],
            base_url=d["base_url"])
        assert ms < 100, f"Create task: {ms:.0f}ms"

    def test_get_config_under_100ms(self, perf_daemon):
        d = perf_daemon
        _, _, ms = _timed_api("GET", "/api/config", token=d["token"],
            base_url=d["base_url"])
        assert ms < 100, f"Get config: {ms:.0f}ms"

    def test_list_sprints_under_100ms(self, perf_daemon):
        d = perf_daemon
        _, _, ms = _timed_api("GET", "/api/sprints", token=d["token"],
            base_url=d["base_url"])
        assert ms < 100, f"List sprints: {ms:.0f}ms"

    def test_timer_state_under_100ms(self, perf_daemon):
        d = perf_daemon
        _, _, ms = _timed_api("GET", "/api/timer", token=d["token"],
            base_url=d["base_url"])
        assert ms < 100, f"Timer state: {ms:.0f}ms"

    def test_history_under_100ms(self, perf_daemon):
        d = perf_daemon
        _, _, ms = _timed_api("GET", "/api/history", token=d["token"],
            base_url=d["base_url"])
        assert ms < 100, f"History: {ms:.0f}ms"


# ── API Latency (1000 tasks) ──────────────────────────────────

class TestLatencyLoaded:

    @pytest.fixture(autouse=True, scope="class")
    def _seed_1000_tasks(self, perf_daemon):
        """Create 1000 tasks to simulate a loaded database."""
        d = perf_daemon
        # Batch create for speed
        for i in range(1000):
            _api("POST", "/api/tasks",
                {"title": f"LoadTask_{i}", "project": f"Proj_{i % 10}"},
                token=d["token"], base_url=d["base_url"])

    def test_list_tasks_under_500ms(self, perf_daemon):
        d = perf_daemon
        _, _, ms = _timed_api("GET", "/api/tasks", token=d["token"],
            base_url=d["base_url"])
        assert ms < 500, f"List 1000 tasks: {ms:.0f}ms"

    def test_create_task_under_200ms(self, perf_daemon):
        d = perf_daemon
        _, _, ms = _timed_api("POST", "/api/tasks",
            {"title": "OneMore", "project": "Perf"}, token=d["token"],
            base_url=d["base_url"])
        assert ms < 200, f"Create task (loaded): {ms:.0f}ms"

    def test_search_under_500ms(self, perf_daemon):
        d = perf_daemon
        _, _, ms = _timed_api("GET", "/api/tasks/search?q=LoadTask_500",
            token=d["token"], base_url=d["base_url"])
        assert ms < 500, f"Search (1000 tasks): {ms:.0f}ms"

    def test_single_task_under_200ms(self, perf_daemon):
        d = perf_daemon
        # Get a known task
        _, tasks = _api("GET", "/api/tasks", token=d["token"],
            base_url=d["base_url"])
        if tasks:
            tid = tasks[0]["id"]
            _, _, ms = _timed_api("GET", f"/api/tasks/{tid}",
                token=d["token"], base_url=d["base_url"])
            assert ms < 200, f"Get single task (loaded): {ms:.0f}ms"

    def test_health_still_fast(self, perf_daemon):
        d = perf_daemon
        _, _, ms = _timed_api("GET", "/api/health", base_url=d["base_url"])
        assert ms < 100, f"Health (loaded): {ms:.0f}ms"


# ── Memory Usage ───────────────────────────────────────────────

class TestMemory:

    def test_rss_under_100mb_after_1000_tasks(self, perf_daemon):
        """Daemon RSS should stay under 100MB after 1000 tasks."""
        pid = perf_daemon["proc"].pid
        try:
            with open(f"/proc/{pid}/status") as f:
                for line in f:
                    if line.startswith("VmRSS:"):
                        rss_kb = int(line.split()[1])
                        rss_mb = rss_kb / 1024
                        assert rss_mb < 100, f"RSS: {rss_mb:.1f}MB (limit: 100MB)"
                        return
        except FileNotFoundError:
            pytest.skip("Cannot read /proc — not on Linux")

    def test_rss_under_50mb_baseline(self, perf_daemon):
        """Even with 1000 tasks, RSS should be well under 50MB for a Rust daemon."""
        pid = perf_daemon["proc"].pid
        try:
            with open(f"/proc/{pid}/status") as f:
                for line in f:
                    if line.startswith("VmRSS:"):
                        rss_kb = int(line.split()[1])
                        rss_mb = rss_kb / 1024
                        # Rust + SQLite should be very lean
                        assert rss_mb < 50, f"RSS: {rss_mb:.1f}MB (limit: 50MB)"
                        return
        except FileNotFoundError:
            pytest.skip("Cannot read /proc — not on Linux")


# ── Concurrent Throughput ──────────────────────────────────────

class TestThroughput:

    def test_50_parallel_reads_under_10s(self, perf_daemon):
        d = perf_daemon
        t0 = time.perf_counter()
        with ThreadPoolExecutor(max_workers=50) as pool:
            futures = [
                pool.submit(_api, "GET", "/api/tasks",
                    None, d["token"], d["base_url"])
                for _ in range(50)
            ]
            results = [f.result() for f in as_completed(futures)]
        elapsed = time.perf_counter() - t0
        assert elapsed < 10, f"50 parallel reads: {elapsed:.1f}s (limit: 10s)"
        assert all(r[0] == 200 for r in results)

    def test_50_parallel_creates_under_10s(self, perf_daemon):
        d = perf_daemon
        t0 = time.perf_counter()
        with ThreadPoolExecutor(max_workers=50) as pool:
            futures = [
                pool.submit(_api, "POST", "/api/tasks",
                    {"title": f"Thr_{i}", "project": "Throughput"},
                    d["token"], d["base_url"])
                for i in range(50)
            ]
            results = [f.result() for f in as_completed(futures)]
        elapsed = time.perf_counter() - t0
        assert elapsed < 10, f"50 parallel creates: {elapsed:.1f}s (limit: 10s)"
        assert all(r[0] == 201 for r in results)

    def test_mixed_workload_under_10s(self, perf_daemon):
        """50 requests: mix of GET, POST, PUT."""
        d = perf_daemon
        # Create a task to update
        _, task = _api("POST", "/api/tasks",
            {"title": "MixTarget", "project": "Mix"},
            d["token"], d["base_url"])
        tid = task["id"]

        def mixed_op(i):
            if i % 3 == 0:
                return _api("GET", "/api/tasks", None, d["token"], d["base_url"])
            elif i % 3 == 1:
                return _api("POST", "/api/tasks",
                    {"title": f"Mix_{i}", "project": "Mix"},
                    d["token"], d["base_url"])
            else:
                return _api("PUT", f"/api/tasks/{tid}",
                    {"title": f"Updated_{i}"},
                    d["token"], d["base_url"])

        t0 = time.perf_counter()
        with ThreadPoolExecutor(max_workers=50) as pool:
            futures = [pool.submit(mixed_op, i) for i in range(50)]
            results = [f.result() for f in as_completed(futures)]
        elapsed = time.perf_counter() - t0
        assert elapsed < 10, f"Mixed workload: {elapsed:.1f}s (limit: 10s)"
        # All should succeed (200 or 201)
        assert all(r[0] in (200, 201) for r in results)

    def test_p99_latency_under_500ms(self, perf_daemon):
        """99th percentile latency for 100 sequential requests."""
        d = perf_daemon
        latencies = []
        for _ in range(100):
            _, _, ms = _timed_api("GET", "/api/health", base_url=d["base_url"])
            latencies.append(ms)
        latencies.sort()
        p99 = latencies[98]  # 99th percentile
        assert p99 < 500, f"P99 latency: {p99:.0f}ms (limit: 500ms)"
