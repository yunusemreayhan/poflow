"""Input validation tests: verify every create/update endpoint rejects bad input.

Parametrized tests for missing required fields, wrong types, and boundary values.
"""
import pytest
from helpers import H


# ── Missing Required Fields ────────────────────────────────────

MISSING_FIELD_CASES = [
    ("POST", "/api/tasks", {}, "task without title"),
    ("POST", "/api/tasks", {"project": "P"}, "task without title"),
    ("POST", "/api/sprints", {}, "sprint without name"),
    ("POST", "/api/labels", {}, "label without name"),
    ("POST", "/api/teams", {}, "team without name"),
    ("POST", "/api/rooms", {}, "room without name"),
    ("POST", "/api/auth/login", {}, "login without credentials"),
    ("POST", "/api/auth/login", {"username": "root"}, "login without password"),
    ("POST", "/api/auth/login", {"password": "x"}, "login without username"),
    ("POST", "/api/auth/register", {}, "register without credentials"),
    ("POST", "/api/auth/register", {"username": "x"}, "register without password"),
    ("POST", "/api/auth/register", {"password": "x"}, "register without username"),
]


class TestMissingFields:

    @pytest.mark.parametrize("method,path,body,desc", MISSING_FIELD_CASES)
    def test_rejects_missing_fields(self, logged_in, method, path, body, desc):
        h = H()
        code, _ = h.api_status(method, path, body)
        assert code in (400, 401, 422), f"{desc}: got {code}"


# ── Wrong Types ────────────────────────────────────────────────

WRONG_TYPE_CASES = [
    ("POST", "/api/tasks", {"title": 123}, "title as number"),
    ("POST", "/api/tasks", {"title": "OK", "priority": "high"}, "priority as string"),
    ("POST", "/api/tasks", {"title": "OK", "estimated": "five"}, "estimated as string"),
    ("POST", "/api/sprints", {"name": 123}, "sprint name as number"),
    ("POST", "/api/labels", {"name": 123}, "label name as number"),
    ("POST", "/api/teams", {"name": 123}, "team name as number"),
]


class TestWrongTypes:

    @pytest.mark.parametrize("method,path,body,desc", WRONG_TYPE_CASES)
    def test_rejects_or_coerces_wrong_types(self, logged_in, method, path, body, desc):
        h = H()
        code, _ = h.api_status(method, path, body)
        # Should either reject (400/422) or coerce successfully (201)
        assert code in (201, 400, 422), f"{desc}: got {code}"


# ── Boundary Values ────────────────────────────────────────────

BOUNDARY_CASES = [
    ("POST", "/api/tasks", {"title": ""}, "empty title"),
    ("POST", "/api/tasks", {"title": " "}, "whitespace title"),
    ("POST", "/api/tasks", {"title": "x" * 10000}, "very long title"),
    ("POST", "/api/sprints", {"name": ""}, "empty sprint name"),
    ("POST", "/api/sprints", {"name": " "}, "whitespace sprint name"),
    ("POST", "/api/sprints", {"name": "x" * 10000}, "very long sprint name"),
    ("POST", "/api/labels", {"name": ""}, "empty label name"),
    ("POST", "/api/labels", {"name": "x" * 10000}, "very long label name"),
    ("POST", "/api/teams", {"name": ""}, "empty team name"),
    ("POST", "/api/rooms", {"name": ""}, "empty room name"),
    ("POST", "/api/tasks", {"title": "OK", "priority": -1}, "negative priority"),
    ("POST", "/api/tasks", {"title": "OK", "priority": 0}, "zero priority"),
    ("POST", "/api/tasks", {"title": "OK", "priority": 999}, "huge priority"),
    ("POST", "/api/tasks", {"title": "OK", "estimated": -1}, "negative estimated"),
    ("POST", "/api/tasks", {"title": "OK", "estimated": 0}, "zero estimated"),
    ("POST", "/api/tasks", {"title": "OK", "estimated": 99999}, "huge estimated"),
]


class TestBoundaryValues:

    @pytest.mark.parametrize("method,path,body,desc", BOUNDARY_CASES)
    def test_handles_boundary_values(self, logged_in, method, path, body, desc):
        h = H()
        code, _ = h.api_status(method, path, body)
        # Should either accept (201) or reject gracefully (400/422) — never 500
        assert code in (200, 201, 400, 422), f"{desc}: got {code}"
