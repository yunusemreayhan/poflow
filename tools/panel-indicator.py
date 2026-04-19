#!/usr/bin/env python3
"""Poflow MATE panel indicator — shows timer countdown in the system tray."""

import json
import os
import signal
import urllib.request
import urllib.error

import gi
gi.require_version("AppIndicator3", "0.1")
gi.require_version("Gtk", "3.0")
from gi.repository import AppIndicator3, Gtk, GLib

BASE_URL = os.environ.get("POFLOW_URL", "http://127.0.0.1:9090")
POLL_MS = 1000
TOKEN = None

SOUND_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "sounds")
# Fallback: installed location
if not os.path.isdir(SOUND_DIR):
    SOUND_DIR = "/usr/share/poflow/sounds"

SOUNDS = {
    "work_start":  os.path.join(SOUND_DIR, "work-start.ogg"),
    "work_end":    os.path.join(SOUND_DIR, "work-end.ogg"),
    "break_end":   os.path.join(SOUND_DIR, "break-end.ogg"),
    "tick":        os.path.join(SOUND_DIR, "tick.ogg"),
}


def play_sound(name):
    path = SOUNDS.get(name, "")
    if os.path.isfile(path):
        try:
            import subprocess
            subprocess.Popen(["paplay", path], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        except Exception:
            pass


ICONS = {
    "Idle":       "appointment-soon",
    "Work":       "media-record",
    "ShortBreak": "media-playback-pause",
    "LongBreak":  "media-playback-pause",
}

LABELS = {
    "Idle":       "⏸",
    "Work":       "🍅",
    "ShortBreak": "☕",
    "LongBreak":  "🛋",
}


def fmt_time(secs):
    m, s = divmod(max(secs, 0), 60)
    return f"{m}:{s:02d}"


def api_get(path):
    hdrs = {"X-Requested-With": "indicator"}
    if TOKEN:
        hdrs["Authorization"] = f"Bearer {TOKEN}"
    req = urllib.request.Request(f"{BASE_URL}{path}", headers=hdrs)
    resp = urllib.request.urlopen(req, timeout=3)
    return json.loads(resp.read())


def try_login():
    """Try to login with saved auth or env credentials."""
    global TOKEN
    # Try env var
    user = os.environ.get("POFLOW_USER", "root")
    pw = os.environ.get("POFLOW_PASSWORD", "root")
    try:
        data = json.dumps({"username": user, "password": pw}).encode()
        req = urllib.request.Request(
            f"{BASE_URL}/api/auth/login", data=data,
            headers={"Content-Type": "application/json", "X-Requested-With": "indicator"})
        resp = urllib.request.urlopen(req, timeout=3)
        TOKEN = json.loads(resp.read())["token"]
        return True
    except Exception:
        return False


class PoflowIndicator:
    def __init__(self):
        self.indicator = AppIndicator3.Indicator.new(
            "poflow-timer",
            "appointment-soon",
            AppIndicator3.IndicatorCategory.APPLICATION_STATUS,
        )
        self.indicator.set_status(AppIndicator3.IndicatorStatus.ACTIVE)
        self.indicator.set_label("⏸ Idle", "")
        self.indicator.set_title("Poflow Timer")

        # Menu
        menu = Gtk.Menu()

        self.status_item = Gtk.MenuItem(label="Idle")
        self.status_item.set_sensitive(False)
        menu.append(self.status_item)

        menu.append(Gtk.SeparatorMenuItem())

        start_item = Gtk.MenuItem(label="Start Work")
        start_item.connect("activate", lambda _: self._action("/api/timer/start", {"task_id": None}))
        menu.append(start_item)

        pause_item = Gtk.MenuItem(label="Pause")
        pause_item.connect("activate", lambda _: self._action("/api/timer/pause"))
        menu.append(pause_item)

        resume_item = Gtk.MenuItem(label="Resume")
        resume_item.connect("activate", lambda _: self._action("/api/timer/resume"))
        menu.append(resume_item)

        stop_item = Gtk.MenuItem(label="Stop")
        stop_item.connect("activate", lambda _: self._action("/api/timer/stop"))
        menu.append(stop_item)

        skip_item = Gtk.MenuItem(label="Skip")
        skip_item.connect("activate", lambda _: self._action("/api/timer/skip"))
        menu.append(skip_item)

        menu.append(Gtk.SeparatorMenuItem())

        quit_item = Gtk.MenuItem(label="Quit Indicator")
        quit_item.connect("activate", lambda _: Gtk.main_quit())
        menu.append(quit_item)

        menu.show_all()
        self.indicator.set_menu(menu)

        self.last_phase = None
        GLib.timeout_add(POLL_MS, self._poll)

    def _action(self, path, body=None):
        try:
            data = json.dumps(body or {}).encode()
            req = urllib.request.Request(
                f"{BASE_URL}{path}", data=data,
                headers={"Content-Type": "application/json",
                         "X-Requested-With": "indicator",
                         "Authorization": f"Bearer {TOKEN}" if TOKEN else ""})
            urllib.request.urlopen(req, timeout=3)
        except Exception:
            pass

    def _poll(self):
        global TOKEN
        try:
            state = api_get("/api/timer")
        except urllib.error.HTTPError as e:
            if e.code == 401 and try_login():
                return True
            self.indicator.set_label("⚠ Auth", "")
            return True
        except Exception:
            self.indicator.set_label("⚠ Offline", "")
            return True

        phase = state.get("phase", "Idle")
        status = state.get("status", "Idle")
        elapsed = state.get("elapsed_s", 0)
        duration = state.get("duration_s", 0)
        remaining = duration - elapsed
        daily = state.get("daily_completed", 0)
        goal = state.get("daily_goal", 0)

        icon_name = ICONS.get(phase, "appointment-soon")
        emoji = LABELS.get(phase, "⏸")

        if status == "Idle":
            label = f"⏸ {daily}/{goal}"
            detail = f"Idle — {daily}/{goal} sessions today"
        elif status == "Paused":
            label = f"⏸ {fmt_time(remaining)}"
            detail = f"{phase} paused — {fmt_time(remaining)} left"
        else:
            label = f"{emoji} {fmt_time(remaining)}"
            detail = f"{phase} — {fmt_time(remaining)} left ({daily}/{goal})"

        self.indicator.set_icon_full(icon_name, phase)
        self.indicator.set_label(label, "")
        self.status_item.set_label(detail)

        # Notify and play sound on phase change
        if self.last_phase is not None and self.last_phase != phase:
            if phase == "Work" and self.last_phase in ("ShortBreak", "LongBreak"):
                play_sound("break_end")
                play_sound("work_start")
            elif phase == "Work" and self.last_phase == "Idle":
                play_sound("work_start")
            elif phase in ("ShortBreak", "LongBreak") and self.last_phase == "Work":
                play_sound("work_end")
            if status != "Idle":
                try:
                    import subprocess
                    msg = f"{'Work time!' if phase == 'Work' else 'Break time!'}"
                    subprocess.Popen(["notify-send", "Poflow", msg, "-i", icon_name])
                except Exception:
                    pass

        # Tick sound in last 5 seconds
        if status == "Running" and 0 < remaining <= 5:
            play_sound("tick")
        self.last_phase = phase

        return True


def main():
    signal.signal(signal.SIGINT, signal.SIG_DFL)
    if not TOKEN:
        try_login()
    PoflowIndicator()
    Gtk.main()


if __name__ == "__main__":
    main()
