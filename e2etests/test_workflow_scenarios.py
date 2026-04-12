"""Realistic end-to-end workflow scenarios that chain multiple operations.

Each test class represents a real user story — the kind of multi-step
flow that breaks when individual endpoints work but integration doesn't.
"""
import time
import pytest
from helpers import H, _api, _api_status
import harness


def _get_user_id(username):
    """Get user_id from admin user list."""
    root = H()
    users = root.admin_users()
    return next(u["id"] for u in users if u["username"] == username)


# ── Scenario 1: New Developer Onboarding ───────────────────────

class TestNewDeveloperOnboarding:
    """Register → login → create task → start timer → complete → check history."""

    def test_full_onboarding_flow(self, logged_in):
        # Register new developer
        dev = H.register("new_dev_onboard")
        assert len(dev.token) > 20

        # Create first task
        task = dev.create_task("Setup dev environment", project="Onboarding",
                               estimated=2, priority=2)
        assert task["id"] > 0
        assert task["title"] == "Setup dev environment"

        # Start timer on the task
        dev.start_timer(task["id"])
        state = dev.timer_state()
        assert state["status"] == "Running"

        # Work for a moment, then stop
        time.sleep(1)
        dev.stop_timer()
        state = dev.timer_state()
        assert state["status"] == "Idle"

        # Check history shows at least 1 session
        hist = dev.history()
        assert len(hist) >= 1
        latest = hist[0]
        assert latest.get("task_id") == task["id"] or latest.get("status") in ("completed", "interrupted")

    def test_first_task_appears_in_list(self, logged_in):
        dev = H.register("new_dev_list")
        dev.create_task("My first task", project="Welcome")
        tasks = dev.list_tasks()
        titles = [t["title"] for t in tasks]
        assert "My first task" in titles

    def test_new_user_can_comment(self, logged_in):
        dev = H.register("new_dev_comment")
        task = dev.create_task("Commentable task")
        comment = dev.add_comment(task["id"], "My first comment!")
        assert comment["content"] == "My first comment!"
        comments = dev.list_comments(task["id"])
        assert len(comments) == 1


# ── Scenario 2: Sprint Planning Meeting ────────────────────────

class TestSprintPlanningMeeting:
    """Create sprint → create tasks → assign → prioritize → start → move to WIP."""

    def test_full_planning_flow(self, logged_in):
        h = H()

        # Create sprint for the upcoming week
        sprint = h.create_sprint("Sprint 42 - Auth Module")
        assert sprint["status"] == "planning"

        # Create 10 tasks with varying priorities
        tasks = []
        for i in range(10):
            t = h.create_task(f"AUTH-{i+1}: Task {i+1}",
                              project="Auth", estimated=i % 5 + 1,
                              priority=(i % 4) + 1)
            tasks.append(t)
        assert len(tasks) == 10

        # Add all tasks to sprint
        task_ids = [t["id"] for t in tasks]
        h.add_sprint_tasks(sprint["id"], task_ids)
        sprint_tasks = h.sprint_tasks(sprint["id"])
        assert len(sprint_tasks) == 10

        # Start the sprint
        h.start_sprint(sprint["id"])
        sprints = h.list_sprints()
        active = [s for s in sprints if s["id"] == sprint["id"]]
        assert active[0]["status"] == "active"

        # Move 3 highest-priority tasks to in_progress
        for t in tasks[:3]:
            h.set_task_status(t["id"], "in_progress")

        # Verify sprint tasks still sum to 10
        sprint_tasks = h.sprint_tasks(sprint["id"])
        assert len(sprint_tasks) == 10
        wip = [t for t in sprint_tasks if t["status"] == "in_progress"]
        assert len(wip) == 3

    def test_sprint_with_labels(self, logged_in):
        h = H()
        sprint = h.create_sprint("Labeled Sprint")
        bug = h.create_label("bug")
        feature = h.create_label("feature")

        t1 = h.create_task("Fix login crash", project="Auth")
        t2 = h.create_task("Add OAuth support", project="Auth")
        h.assign_label(t1["id"], bug["id"])
        h.assign_label(t2["id"], feature["id"])

        h.add_sprint_tasks(sprint["id"], [t1["id"], t2["id"]])
        h.start_sprint(sprint["id"])

        # Verify labels persisted
        t1_labels = h.task_labels(t1["id"])
        assert any(l["name"] == "bug" for l in t1_labels)


# ── Scenario 3: Code Review / Estimation Day ──────────────────

class TestCodeReviewDay:
    """Create room → 3 users join → vote on 5 tasks → reveal → verify estimates."""

    def test_full_estimation_flow(self, logged_in):
        lead = H()
        dev1 = H.register("reviewer_1")
        dev2 = H.register("reviewer_2")

        # Create estimation room
        room = lead.create_room("Sprint 42 Estimation")
        lead.join_room(room["id"])
        dev1.join_room(room["id"])
        dev2.join_room(room["id"])

        # Create 5 tasks to estimate
        tasks = [lead.create_task(f"Estimate_{i}", project="Est",
                                  estimated=0) for i in range(5)]

        estimates = []
        for task in tasks:
            # Start voting on this task
            lead.start_voting(room["id"], task["id"])

            # All 3 vote
            lead.vote(room["id"], 5)
            dev1.vote(room["id"], 8)
            dev2.vote(room["id"], 5)

            # Reveal votes
            lead.reveal_votes(room["id"])

            # Accept the estimate (median/average)
            lead.accept_estimate(room["id"], 5.0)
            estimates.append(5.0)

        # Verify room has vote history
        room_state = lead.get_room(room["id"])
        assert isinstance(room_state, dict)

    def test_estimation_with_outlier(self, logged_in):
        lead = H()
        dev1 = H.register("outlier_dev")

        room = lead.create_room("Outlier Room")
        lead.join_room(room["id"])
        dev1.join_room(room["id"])

        task = lead.create_task("Outlier task", project="Est")
        lead.start_voting(room["id"], task["id"])

        # Wildly different estimates
        lead.vote(room["id"], 1)
        dev1.vote(room["id"], 100)

        lead.reveal_votes(room["id"])
        # Accept a compromise
        lead.accept_estimate(room["id"], 13.0)


# ── Scenario 4: End of Sprint ─────────────────────────────────

class TestEndOfSprint:
    """Burn tasks → complete sprint → verify burndown → carryover to new sprint."""

    def test_full_sprint_completion(self, logged_in):
        h = H()

        # Setup sprint with tasks
        sprint = h.create_sprint("Sprint 41 - Closing")
        tasks = [h.create_task(f"Close_{i}", project="Close",
                               estimated=3) for i in range(5)]
        task_ids = [t["id"] for t in tasks]
        h.add_sprint_tasks(sprint["id"], task_ids)
        h.start_sprint(sprint["id"])

        # Complete 3 tasks, leave 2 incomplete
        for t in tasks[:3]:
            h.set_task_status(t["id"], "done")
            h.burn(sprint["id"], t["id"], points=3.0, hours=1.5)

        # Burn partial on incomplete tasks
        h.burn(sprint["id"], tasks[3]["id"], points=1.0, hours=0.5)

        # Complete the sprint
        h.complete_sprint(sprint["id"])
        sprints = h.list_sprints()
        closed = [s for s in sprints if s["id"] == sprint["id"]]
        assert closed[0]["status"] in ("completed", "closed")

        # Verify burn data
        burns = h.sprint_burns(sprint["id"])
        total_points = sum(b.get("points", 0) for b in burns)
        assert total_points == 10.0  # 3*3 + 1

        # Carryover: create new sprint with incomplete tasks
        new_sprint = h.create_sprint("Sprint 42 - Carryover")
        incomplete = [t["id"] for t in tasks[3:]]
        h.add_sprint_tasks(new_sprint["id"], incomplete)
        new_tasks = h.sprint_tasks(new_sprint["id"])
        assert len(new_tasks) == 2

    def test_burndown_has_data(self, logged_in):
        h = H()
        sprint = h.create_sprint("Burndown Sprint")
        t = h.create_task("BurndownTask", estimated=5)
        h.add_sprint_tasks(sprint["id"], [t["id"]])
        h.start_sprint(sprint["id"])
        h.burn(sprint["id"], t["id"], points=2.0, hours=1.0)
        burndown = h.sprint_burndown(sprint["id"])
        assert isinstance(burndown, (list, dict))


# ── Scenario 5: Team Lead Dashboard ───────────────────────────

class TestTeamLeadDashboard:
    """Create team → add members → each creates tasks → verify counts."""

    def test_full_team_workflow(self, logged_in):
        lead = H()

        # Create team
        team = lead.create_team("Backend Team")
        assert team["id"] > 0

        # Register 3 developers
        devs = [H.register(f"backend_dev_{i}") for i in range(3)]

        # Add them to the team
        for dev in devs:
            uid = _get_user_id(dev.user)
            lead.add_team_member(team["id"], uid)

        # Each dev creates 3 tasks
        all_tasks = []
        for dev in devs:
            for j in range(3):
                t = dev.create_task(f"{dev.user}_task_{j}", project="Backend")
                all_tasks.append(t)

        assert len(all_tasks) == 9

        # Lead can see all tasks
        tasks = lead.list_tasks()
        team_tasks = [t for t in tasks if t["project"] == "Backend"]
        assert len(team_tasks) >= 9

        # Check stats
        stats = lead.stats()
        assert isinstance(stats, (list, dict))

    def test_team_member_isolation(self, logged_in):
        """Non-root users can only modify their own tasks."""
        dev_a = H.register("team_iso_a")
        dev_b = H.register("team_iso_b")

        task_a = dev_a.create_task("A's task", project="Iso")
        # B cannot delete A's task
        code, _ = dev_b.api_status("DELETE", f"/api/tasks/{task_a['id']}")
        assert code == 403

    def test_team_tasks_have_correct_owners(self, logged_in):
        dev = H.register("team_owner_check")
        t = dev.create_task("Owner check", project="Own")
        detail = dev.get_task(t["id"])
        task = detail.get("task", detail)
        assert task["user"] == "team_owner_check"


# ── Scenario 6: Cleanup Day ───────────────────────────────────

class TestCleanupDay:
    """Archive old tasks → delete labels → remove completed sprints."""

    def test_full_cleanup_flow(self, logged_in):
        h = H()

        # Create old tasks and mark them done
        old_tasks = [h.create_task(f"Old_{i}", project="Legacy") for i in range(5)]
        for t in old_tasks:
            h.set_task_status(t["id"], "done")

        # Archive them
        for t in old_tasks:
            h.set_task_status(t["id"], "archived")

        # Verify they're archived
        tasks = h.list_tasks()
        archived = [t for t in tasks if t["title"].startswith("Old_")
                    and t["status"] == "archived"]
        assert len(archived) == 5

        # Delete unused labels
        lbl1 = h.create_label("deprecated_feature")
        lbl2 = h.create_label("wontfix")
        h.api_status("DELETE", f"/api/labels/{lbl1['id']}")
        h.api_status("DELETE", f"/api/labels/{lbl2['id']}")
        labels = h.list_labels()
        label_names = [l["name"] for l in labels]
        assert "deprecated_feature" not in label_names
        assert "wontfix" not in label_names

        # Remove completed sprint
        sprint = h.create_sprint("Old Sprint")
        h.start_sprint(sprint["id"])
        h.complete_sprint(sprint["id"])
        h.api_status("DELETE", f"/api/sprints/{sprint['id']}")
        sprints = h.list_sprints()
        sprint_ids = [s["id"] for s in sprints]
        assert sprint["id"] not in sprint_ids

    def test_soft_delete_and_restore(self, logged_in):
        h = H()
        tasks = [h.create_task(f"SoftDel_{i}", project="Cleanup") for i in range(3)]

        # Soft delete all
        for t in tasks:
            h.delete_task(t["id"])

        # Verify in trash
        trash = h.list_trash()
        trashed = [t for t in trash if t["title"].startswith("SoftDel_")]
        assert len(trashed) == 3

        # Restore one
        h.restore_task(tasks[0]["id"])
        active = h.list_tasks()
        restored = [t for t in active if t["title"] == "SoftDel_0"]
        assert len(restored) == 1

        # Purge the rest
        for t in tasks[1:]:
            h.purge_task(t["id"])
        trash2 = h.list_trash()
        remaining = [t for t in trash2 if t["title"].startswith("SoftDel_")]
        assert len(remaining) == 0

    def test_cleanup_comments_on_deleted_task(self, logged_in):
        h = H()
        t = h.create_task("CommentCleanup", project="Cleanup")
        h.add_comment(t["id"], "Comment 1")
        h.add_comment(t["id"], "Comment 2")
        h.add_comment(t["id"], "Comment 3")
        assert len(h.list_comments(t["id"])) == 3

        # Delete the task
        h.delete_task(t["id"])
        # Comments should still be accessible or task in trash
        trash = h.list_trash()
        assert any(tr["id"] == t["id"] for tr in trash)


# ── Scenario 7: Multi-Sprint Velocity ─────────────────────────

class TestMultiSprintVelocity:
    """Run 3 sprints, track velocity across them."""

    def test_velocity_across_sprints(self, logged_in):
        h = H()
        velocities = []

        for sprint_num in range(3):
            sprint = h.create_sprint(f"Velocity Sprint {sprint_num}")
            tasks = [h.create_task(f"V{sprint_num}_{i}", project="Velocity",
                                   estimated=2) for i in range(4)]
            h.add_sprint_tasks(sprint["id"], [t["id"] for t in tasks])
            h.start_sprint(sprint["id"])

            # Complete varying number of tasks per sprint
            completed = sprint_num + 2  # 2, 3, 4
            for t in tasks[:completed]:
                h.set_task_status(t["id"], "done")
                h.burn(sprint["id"], t["id"], points=2.0, hours=1.0)

            h.complete_sprint(sprint["id"])
            burns = h.sprint_burns(sprint["id"])
            velocity = sum(b.get("points", 0) for b in burns)
            velocities.append(velocity)

        # Velocity should increase: 4, 6, 8
        assert velocities == [4.0, 6.0, 8.0]


# ── Scenario 8: Timer Workflow ─────────────────────────────────

class TestTimerWorkflow:
    """Start → pause → resume → stop → verify session recorded."""

    def test_start_pause_resume_stop(self, logged_in):
        h = H()
        task = h.create_task("Timer workflow task")

        # Start
        h.start_timer(task["id"])
        assert h.timer_state()["status"] == "Running"

        # Pause
        time.sleep(0.5)
        h.pause_timer()
        assert h.timer_state()["status"] == "Paused"

        # Resume
        h.resume_timer()
        assert h.timer_state()["status"] == "Running"

        # Stop
        time.sleep(0.5)
        h.stop_timer()
        assert h.timer_state()["status"] == "Idle"

        # History should have the session
        hist = h.history()
        assert len(hist) >= 1

    def test_timer_without_task(self, logged_in):
        """Timer can run without a specific task."""
        h = H()
        h.start_timer()
        assert h.timer_state()["status"] == "Running"
        time.sleep(0.5)
        h.stop_timer()
        assert h.timer_state()["status"] == "Idle"


# ── Scenario 9: Cross-User Collaboration ──────────────────────

class TestCrossUserCollaboration:
    """Multiple users work on the same sprint, commenting on each other's tasks."""

    def test_shared_sprint(self, logged_in):
        lead = H()
        dev1 = H.register("collab_dev1")
        dev2 = H.register("collab_dev2")

        sprint = lead.create_sprint("Collab Sprint")

        # Each user creates tasks
        t1 = dev1.create_task("Dev1 feature", project="Collab")
        t2 = dev2.create_task("Dev2 feature", project="Collab")
        t3 = lead.create_task("Lead feature", project="Collab")

        # Lead adds all to sprint
        lead.add_sprint_tasks(sprint["id"], [t1["id"], t2["id"], t3["id"]])
        lead.start_sprint(sprint["id"])

        # Users comment on each other's tasks (read is team-visible)
        dev1.add_comment(t2["id"], "Looks good, dev2!")
        dev2.add_comment(t1["id"], "Nice work, dev1!")
        lead.add_comment(t1["id"], "Approved")
        lead.add_comment(t2["id"], "Approved")

        # Verify comments
        t1_comments = lead.list_comments(t1["id"])
        assert len(t1_comments) == 2  # dev2 + lead
        t2_comments = lead.list_comments(t2["id"])
        assert len(t2_comments) == 2  # dev1 + lead

        # Sprint has all 3 tasks
        assert len(lead.sprint_tasks(sprint["id"])) == 3

    def test_task_visibility(self, logged_in):
        """All users can see all tasks (team-visible)."""
        owner = H.register("vis_owner")
        viewer = H.register("vis_viewer")

        task = owner.create_task("Visible task", project="Vis")
        # Viewer can read it
        code, body = viewer.api_status("GET", f"/api/tasks/{task['id']}")
        assert code == 200


# ── Scenario 10: Epic Workflow ─────────────────────────────────

class TestEpicWorkflow:
    """Create epic → add tasks → track progress."""

    def test_epic_with_tasks(self, logged_in):
        h = H()
        epic = h.create_epic("Authentication Overhaul")
        assert epic["id"] > 0

        # Create tasks under the epic
        tasks = []
        for name in ["OAuth2 provider", "JWT refresh", "Password reset",
                      "2FA setup", "Session management"]:
            t = h.create_task(name, project="Auth", estimated=3)
            tasks.append(t)

        # Complete some tasks
        for t in tasks[:3]:
            h.set_task_status(t["id"], "done")

        # Verify task statuses
        all_tasks = h.list_tasks()
        auth_done = [t for t in all_tasks if t["project"] == "Auth"
                     and t["status"] == "done"
                     and t["title"] in ["OAuth2 provider", "JWT refresh", "Password reset"]]
        assert len(auth_done) >= 3
