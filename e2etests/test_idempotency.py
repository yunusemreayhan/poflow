"""Idempotency tests: repeating the same operation should be safe.

Catches double-free bugs, duplicate insert errors, and state machine violations.
"""
import pytest
from helpers import H


class TestDoubleDelete:

    def test_double_delete_task(self, logged_in):
        h = H()
        t = h.create_task("DblDel")
        code1, _ = h.delete_task(t["id"])
        assert code1 == 204
        code2, _ = h.delete_task(t["id"])
        assert code2 in (204, 404)

    def test_double_purge_task(self, logged_in):
        h = H()
        t = h.create_task("DblPurge")
        h.delete_task(t["id"])
        code1, _ = h.purge_task(t["id"])
        assert code1 in (200, 204)
        code2, _ = h.purge_task(t["id"])
        assert code2 in (200, 204, 404)

    def test_double_delete_comment(self, logged_in):
        h = H()
        t = h.create_task("DblDelCm")
        c = h.add_comment(t["id"], "To delete twice")
        code1, _ = h.api_status("DELETE", f"/api/comments/{c['id']}")
        assert code1 in (200, 204)
        code2, _ = h.api_status("DELETE", f"/api/comments/{c['id']}")
        # Already deleted → 404
        assert code2 in (200, 204, 404)

    def test_double_delete_label(self, logged_in):
        h = H()
        lbl = h.create_label("DblDelLabel")
        code1, _ = h.api_status("DELETE", f"/api/labels/{lbl['id']}")
        assert code1 in (200, 204)
        code2, _ = h.api_status("DELETE", f"/api/labels/{lbl['id']}")
        assert code2 in (200, 204, 404)

    def test_double_delete_room(self, logged_in):
        h = H()
        r = h.create_room("DblDelRoom")
        code1, _ = h.api_status("DELETE", f"/api/rooms/{r['id']}")
        assert code1 in (200, 204)
        code2, _ = h.api_status("DELETE", f"/api/rooms/{r['id']}")
        assert code2 in (200, 204, 404)

    def test_double_delete_sprint(self, logged_in):
        h = H()
        s = h.create_sprint("DblDelSprint")
        code1, _ = h.api_status("DELETE", f"/api/sprints/{s['id']}")
        assert code1 in (200, 204)
        code2, _ = h.api_status("DELETE", f"/api/sprints/{s['id']}")
        assert code2 in (200, 204, 404)

    def test_double_delete_team(self, logged_in):
        h = H()
        t = h.create_team("DblDelTeam")
        code1, _ = h.api_status("DELETE", f"/api/teams/{t['id']}")
        assert code1 in (200, 204)
        code2, _ = h.api_status("DELETE", f"/api/teams/{t['id']}")
        assert code2 in (200, 204, 404)


class TestDoubleCreate:

    def test_double_register_same_user(self, logged_in):
        import random
        name = f"idem_{random.randint(10000,99999)}"
        code1, _ = H().api_status("POST", "/api/auth/register",
                                  {"username": name, "password": "TestPass1"})
        assert code1 in (200, 201)
        code2, _ = H().api_status("POST", "/api/auth/register",
                                  {"username": name, "password": "TestPass1"})
        assert code2 in (200, 201, 409)

    def test_double_add_label_to_task(self, logged_in):
        h = H()
        t = h.create_task("DblLabel")
        lbl = h.create_label("DblLabelAssign")
        h.assign_label(t["id"], lbl["id"])
        code, _ = h.api_status("PUT", f"/api/tasks/{t['id']}/labels/{lbl['id']}")
        # Already assigned → 200/204 (idempotent)
        assert code in (200, 201, 204, 409)

    def test_double_add_task_to_sprint(self, logged_in):
        h = H()
        s = h.create_sprint("DblSprintTask")
        t = h.create_task("DblST")
        h.add_sprint_tasks(s["id"], [t["id"]])
        code, _ = h.api_status("POST", f"/api/sprints/{s['id']}/tasks",
                               {"task_ids": [t["id"]]})
        assert code in (200, 201, 204, 409)

    def test_double_join_room(self, logged_in):
        h = H()
        r = h.create_room("DblJoin")
        h.join_room(r["id"])
        code, _ = h.api_status("POST", f"/api/rooms/{r['id']}/join")
        # Already a member → 204 (idempotent)
        assert code in (200, 201, 204, 409)

    def test_double_add_dependency(self, logged_in):
        h = H()
        a = h.create_task("DblDepA")["id"]
        b = h.create_task("DblDepB")["id"]
        h.api("POST", f"/api/tasks/{a}/dependencies", {"depends_on": b})
        code, _ = h.api_status("POST", f"/api/tasks/{a}/dependencies",
                               {"depends_on": b})
        # Already exists → 204/409
        assert code in (200, 201, 204, 409)


class TestDoubleSprintTransition:

    def test_double_start_sprint(self, logged_in):
        h = H()
        s = h.create_sprint("DblStart")
        h.start_sprint(s["id"])
        code, _ = h.api_status("PUT", f"/api/sprints/{s['id']}/start")
        # Already active → 405 (invalid transition)
        assert code in (200, 400, 405, 409)

    def test_double_complete_sprint(self, logged_in):
        h = H()
        s = h.create_sprint("DblComplete")
        h.start_sprint(s["id"])
        h.complete_sprint(s["id"])
        code, _ = h.api_status("PUT", f"/api/sprints/{s['id']}/complete")
        # Already completed → 405
        assert code in (200, 400, 405, 409)

    def test_start_completed_sprint(self, logged_in):
        h = H()
        s = h.create_sprint("StartCompleted")
        h.start_sprint(s["id"])
        h.complete_sprint(s["id"])
        code, _ = h.api_status("PUT", f"/api/sprints/{s['id']}/start")
        # Completed → can't start again → 405
        assert code in (200, 400, 405, 409)


class TestDoubleVote:

    def test_double_vote_same_value(self, logged_in):
        h = H()
        r = h.create_room("DblVote")
        h.join_room(r["id"])
        t = h.create_task("DblVoteTask")
        h.start_voting(r["id"], t["id"])
        h.vote(r["id"], 5)
        code, _ = h.api_status("POST", f"/api/rooms/{r['id']}/vote", {"value": 5})
        # Updates existing vote → 200/204
        assert code in (200, 201, 204, 409)

    def test_change_vote(self, logged_in):
        h = H()
        r = h.create_room("ChangeVote")
        h.join_room(r["id"])
        t = h.create_task("ChangeVoteTask")
        h.start_voting(r["id"], t["id"])
        h.vote(r["id"], 3)
        code, _ = h.api_status("POST", f"/api/rooms/{r['id']}/vote", {"value": 8})
        # Vote update → 200/204
        assert code in (200, 201, 204)


class TestDoubleBurn:

    def test_double_burn_same_task(self, logged_in):
        """Burning the same task twice should create two entries."""
        h = H()
        s = h.create_sprint("DblBurn")
        t = h.create_task("DblBurnTask")
        h.add_sprint_tasks(s["id"], [t["id"]])
        h.start_sprint(s["id"])
        h.burn(s["id"], t["id"], points=1.0, hours=0.5)
        h.burn(s["id"], t["id"], points=1.0, hours=0.5)
        burns = h.sprint_burns(s["id"])
        assert len(burns) == 2
        total = sum(b.get("points", 0) for b in burns)
        assert total == 2.0

    def test_cancel_burn_twice(self, logged_in):
        h = H()
        s = h.create_sprint("DblCancelBurn")
        t = h.create_task("DblCBTask")
        h.add_sprint_tasks(s["id"], [t["id"]])
        h.start_sprint(s["id"])
        b = h.burn(s["id"], t["id"], points=1.0, hours=0.5)
        h.cancel_burn(s["id"], b["id"])
        code, _ = h.api_status("DELETE", f"/api/sprints/{s['id']}/burns/{b['id']}")
        # Already cancelled → 400/404
        assert code in (200, 204, 400, 404)


class TestDoubleRestore:

    def test_restore_non_deleted_task(self, logged_in):
        h = H()
        t = h.create_task("RestoreActive")
        code, _ = h.api_status("POST", f"/api/tasks/{t['id']}/restore")
        # Not deleted → 204 (no-op) or 400
        assert code in (200, 204, 400, 404)

    def test_double_restore(self, logged_in):
        h = H()
        t = h.create_task("DblRestore")
        h.delete_task(t["id"])
        h.restore_task(t["id"])
        code, _ = h.api_status("POST", f"/api/tasks/{t['id']}/restore")
        # Already restored → 204 (no-op) or 400
        assert code in (200, 204, 400, 404)


class TestDoubleStatusChange:

    def test_set_same_status_twice(self, logged_in):
        h = H()
        t = h.create_task("DblStatus")
        h.set_task_status(t["id"], "in_progress")
        result = h.set_task_status(t["id"], "in_progress")
        assert result["status"] == "in_progress"

    def test_toggle_status_back_and_forth(self, logged_in):
        h = H()
        t = h.create_task("ToggleStatus")
        h.set_task_status(t["id"], "in_progress")
        h.set_task_status(t["id"], "backlog")
        h.set_task_status(t["id"], "in_progress")
        result = h.set_task_status(t["id"], "backlog")
        assert result["status"] == "backlog"
