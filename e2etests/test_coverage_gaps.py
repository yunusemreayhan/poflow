"""Coverage gaps: untested endpoints + attachment lifecycle."""
import pytest
from helpers import H, _api_status
import harness, urllib.request


class TestAttachments:
    """Full attachment lifecycle: upload, list, download, delete."""

    def test_upload_attachment(self, logged_in):
        h = H()
        t = h.create_task("AttTask")
        url = f"{harness.BASE_URL}/api/tasks/{t['id']}/attachments"
        req = urllib.request.Request(url, data=b"hello world",
            headers={"Authorization": f"Bearer {h.token}", "X-Requested-With": "test",
                     "Content-Type": "application/octet-stream", "X-Filename": "test.txt"},
            method="POST")
        resp = urllib.request.urlopen(req, timeout=5)
        assert resp.status == 201
        import json
        att = json.loads(resp.read())
        assert att["filename"] == "test.txt"
        return t["id"], att["id"]

    def test_list_attachments(self, logged_in):
        h = H()
        t = h.create_task("AttList")
        # Upload one
        url = f"{harness.BASE_URL}/api/tasks/{t['id']}/attachments"
        req = urllib.request.Request(url, data=b"data",
            headers={"Authorization": f"Bearer {h.token}", "X-Requested-With": "test",
                     "Content-Type": "application/octet-stream", "X-Filename": "f.bin"},
            method="POST")
        urllib.request.urlopen(req, timeout=5)
        atts = h.api("GET", f"/api/tasks/{t['id']}/attachments")
        assert len(atts) >= 1

    def test_download_attachment(self, logged_in):
        h = H()
        t = h.create_task("AttDl")
        url = f"{harness.BASE_URL}/api/tasks/{t['id']}/attachments"
        req = urllib.request.Request(url, data=b"download me",
            headers={"Authorization": f"Bearer {h.token}", "X-Requested-With": "test",
                     "Content-Type": "application/octet-stream", "X-Filename": "dl.txt"},
            method="POST")
        import json
        att = json.loads(urllib.request.urlopen(req, timeout=5).read())
        # Download
        dl_url = f"{harness.BASE_URL}/api/attachments/{att['id']}/download"
        dl_req = urllib.request.Request(dl_url,
            headers={"Authorization": f"Bearer {h.token}", "X-Requested-With": "test"})
        resp = urllib.request.urlopen(dl_req, timeout=5)
        assert resp.read() == b"download me"

    def test_delete_attachment(self, logged_in):
        h = H()
        t = h.create_task("AttDel")
        url = f"{harness.BASE_URL}/api/tasks/{t['id']}/attachments"
        req = urllib.request.Request(url, data=b"delete me",
            headers={"Authorization": f"Bearer {h.token}", "X-Requested-With": "test",
                     "Content-Type": "application/octet-stream", "X-Filename": "del.txt"},
            method="POST")
        import json
        att = json.loads(urllib.request.urlopen(req, timeout=5).read())
        code, _ = _api_status("DELETE", f"/api/attachments/{att['id']}", token=h.token)
        assert code == 204

    def test_upload_empty_rejected(self, logged_in):
        h = H()
        t = h.create_task("AttEmpty")
        url = f"{harness.BASE_URL}/api/tasks/{t['id']}/attachments"
        req = urllib.request.Request(url, data=b"",
            headers={"Authorization": f"Bearer {h.token}", "X-Requested-With": "test",
                     "Content-Type": "application/octet-stream", "X-Filename": "empty.txt"},
            method="POST")
        try:
            urllib.request.urlopen(req, timeout=5)
            assert False, "Should have failed"
        except urllib.error.HTTPError as e:
            assert e.code == 400

    def test_non_owner_cannot_delete(self, logged_in):
        h = H()
        t = h.create_task("AttPerm")
        url = f"{harness.BASE_URL}/api/tasks/{t['id']}/attachments"
        req = urllib.request.Request(url, data=b"owned",
            headers={"Authorization": f"Bearer {h.token}", "X-Requested-With": "test",
                     "Content-Type": "application/octet-stream", "X-Filename": "own.txt"},
            method="POST")
        import json
        att = json.loads(urllib.request.urlopen(req, timeout=5).read())
        u = H.register("att_user1")
        code, _ = _api_status("DELETE", f"/api/attachments/{att['id']}", token=u.token)
        assert code == 403


class TestTaskSubResources:
    """Untested task sub-resource endpoints."""

    def test_task_burn_total(self, logged_in):
        h = H()
        t = h.create_task("BurnTot", estimated=10)
        s = h.create_sprint("BurnTotSp")
        h.add_sprint_tasks(s["id"], [t["id"]])
        h.start_sprint(s["id"])
        h.burn(s["id"], t["id"], 3.0, 1.5)
        r = h.api("GET", f"/api/tasks/{t['id']}/burn-total")
        assert isinstance(r, dict)

    def test_task_burn_users(self, logged_in):
        h = H()
        t = h.create_task("BurnUsr", estimated=5)
        s = h.create_sprint("BurnUsrSp")
        h.add_sprint_tasks(s["id"], [t["id"]])
        h.start_sprint(s["id"])
        h.burn(s["id"], t["id"], 1.0, 0.5)
        r = h.api("GET", f"/api/tasks/{t['id']}/burn-users")
        assert isinstance(r, list)
        assert "root" in r

    def test_task_sessions(self, logged_in):
        h = H()
        t = h.create_task("SessTask")
        import time
        h.start_timer(t["id"])
        time.sleep(0.5)
        h.stop_timer()
        r = h.api("GET", f"/api/tasks/{t['id']}/sessions")
        assert isinstance(r, list)

    def test_task_votes(self, logged_in):
        h = H()
        t = h.create_task("VoteTask")
        room = h.create_room("VoteRm")
        h.join_room(room["id"])
        h.start_voting(room["id"], t["id"])
        h.vote(room["id"], 8)
        h.reveal_votes(room["id"])
        r = h.api("GET", f"/api/tasks/{t['id']}/votes")
        assert isinstance(r, list)

    def test_tasks_full(self, logged_in):
        h = H()
        h.create_task("FullTask")
        code, r = h.api_status("GET", "/api/tasks/full")
        assert code == 200
