"""Pytest fixtures for flow tests — one daemon per session."""

import pytest
from .harness import Daemon, Client


@pytest.fixture(scope="session")
def daemon():
    """Start an isolated daemon for the entire test session."""
    d = Daemon()
    d.start()
    yield d
    d.stop()


@pytest.fixture(scope="session")
def root(daemon) -> Client:
    """A Client logged in as root, shared across the session."""
    c = Client()
    c.login_root()
    return c


@pytest.fixture()
def fresh_root(daemon) -> Client:
    """A fresh root Client per test (avoids state leakage)."""
    c = Client()
    c.login_root()
    return c
