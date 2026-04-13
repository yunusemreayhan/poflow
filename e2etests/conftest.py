import pytest
from desktop_pilot import TauriWebDriver
import harness
from harness import Daemon, GUI_BINARY, ROOT_PASSWORD, connect_gui_to_daemon, gui_login


@pytest.fixture(scope="session")
def daemon():
    """One daemon per pytest session — fresh DB, random port, temp dir."""
    d = Daemon()
    d.start()
    yield d
    d.stop()


@pytest.fixture(scope="session")
def app(daemon):
    """Tauri GUI connected to the test daemon."""
    d = TauriWebDriver(GUI_BINARY)
    d.start(load_wait=3)
    connect_gui_to_daemon(d)
    yield d
    d.stop()


@pytest.fixture()
def logged_in(app):
    """Ensure the app is logged in as root."""
    try:
        body = app.text(app.find("body"))
    except Exception:
        # Session may be stale — try reloading
        try:
            app.execute_js("location.reload()")
            import time; time.sleep(2)
            body = app.text(app.find("body"))
        except Exception:
            pytest.skip("WebDriver session expired")
    if "Sign In" in body or "sign in" in body.lower():
        connect_gui_to_daemon(app)
        gui_login(app, "root", ROOT_PASSWORD)
    return app
