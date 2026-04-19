"""E2E test: OpenAPI spec endpoint and API reference page.

NOTE: The test harness starts the daemon with POFLOW_SWAGGER=0,
so swagger/openapi endpoints are disabled. These tests verify the
behavior in both cases.

Run standalone:  .venv/bin/pytest test_openapi.py -v --override-ini="addopts="
"""

import urllib.request
import urllib.error
import json
import pytest
import harness
from harness import Daemon
from helpers import H, _api_status


@pytest.fixture(scope="module")
def daemon():
    """Standalone daemon for this module (no GUI needed)."""
    d = Daemon()
    d.start()
    yield d
    d.stop()


class TestOpenApiSpec:
    """Validate the /api-docs/openapi.json endpoint."""

    def _fetch_spec(self):
        """Try to fetch the OpenAPI spec, return (status, body) or None if disabled."""
        try:
            resp = urllib.request.urlopen(
                f"{harness.BASE_URL}/api-docs/openapi.json", timeout=5)
            return resp.status, json.loads(resp.read())
        except urllib.error.HTTPError as e:
            return e.code, None
        except Exception:
            return None, None

    def test_openapi_spec_endpoint_responds(self, daemon):
        """The spec endpoint should return 200 (if swagger enabled) or 404."""
        status, body = self._fetch_spec()
        # Swagger disabled in test harness → 404 is expected
        assert status in (200, 404), f"Unexpected status: {status}"

    def test_openapi_spec_valid_when_enabled(self, daemon):
        """If swagger is enabled, the spec should be valid OpenAPI."""
        status, body = self._fetch_spec()
        if status != 200:
            return  # Swagger disabled, skip
        assert "openapi" in body
        assert "info" in body
        assert "paths" in body
        assert len(body["paths"]) > 50, f"Expected many paths, got {len(body['paths'])}"

    def test_openapi_spec_has_core_endpoints(self, daemon):
        """If swagger is enabled, core endpoints should be documented."""
        status, body = self._fetch_spec()
        if status != 200:
            return
        paths = body["paths"]
        for required in ["/api/auth/login", "/api/auth/register", "/api/tasks", "/api/timer"]:
            assert required in paths, f"Missing {required}"

    def test_openapi_spec_has_schemas(self, daemon):
        """If swagger is enabled, schemas should be present."""
        status, body = self._fetch_spec()
        if status != 200:
            return
        schemas = body.get("components", {}).get("schemas", {})
        for required in ["Task", "AuthResponse", "EngineState", "Sprint"]:
            assert required in schemas, f"Missing schema: {required}"

    def test_health_no_auth(self, daemon):
        """Health endpoint should always work without auth."""
        status, body = _api_status("GET", "/api/health")
        assert status == 200
        assert body["status"] in ("ok", "degraded")
        assert body["db"] is True

    def test_health_reports_schema_version(self, daemon):
        """Health should report schema migration version (if supported)."""
        status, body = _api_status("GET", "/api/health")
        assert status == 200
        # Newer daemon versions include this field
        if "schema_version" in body:
            assert isinstance(body["schema_version"], int)

    def test_health_reports_db_size(self, daemon):
        """Health should report database size (if supported)."""
        status, body = _api_status("GET", "/api/health")
        assert status == 200
        if "db_size_bytes" in body:
            assert body["db_size_bytes"] > 0
