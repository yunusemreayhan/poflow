# BACKLOG v13 — Confirmed Bugs + Business Logic

Audit date: 2026-04-12
Focus: Only confirmed bugs that break things + business workflow improvements

---

## Confirmed Bugs (7)

**B1.** `get_active_timers` holds `engine.states` mutex while doing DB queries (timer.rs:12-19). This blocks the 1-second tick loop and all timer start/stop/pause operations for ALL users while the DB queries run. Fix: collect user IDs + state under lock, drop lock, then query DB.

**B2.** `export.rs:156` — `&t.title[..50]` panics on multi-byte UTF-8 titles (e.g., Japanese, emoji). Any JSON import with a long non-ASCII title crashes the server.

**B3.** `admin.rs:59,112` — `serde_json::to_vec(...).unwrap()` can panic on serialization failure, crashing the server. Should use `.map_err(internal)?`.

**B4.** `edit_comment` parses `created_at` with `%Y-%m-%dT%H:%M:%S%.3f` but if the format doesn't match, the `if let Ok(...)` silently passes — meaning the 15-minute edit window is never enforced. Any comment can be edited forever.

**B5.** `user_hours_report` uses `started_at <= ?` with a date string like `2026-04-12`, but `started_at` is a full ISO timestamp (`2026-04-12T09:30:00`). This excludes the entire end day from the report because `"2026-04-12T..." > "2026-04-12"`.

**B6.** `Dependencies.tsx:43-45` has duplicate `aria-label` attribute on the same element — invalid HTML, causes unpredictable screen reader behavior.

**B7.** `SprintParts.tsx` board view fires N individual API calls to fetch labels (one per task). With 50 tasks in a sprint, that's 50 HTTP requests on every board load. Should use the already-loaded `/api/tasks/full` data.

---

## Business Logic — Sprint & Scrum Workflow (10)

**BL1.** Sprint dashboard for team — when a sprint is active, all team members assigned to sprint tasks should see a shared sprint progress view: burndown chart, task status breakdown, who's working on what. Currently sprint detail is only accessible to the sprint creator.

**BL2.** Sprint task assignment visibility — when tasks are added to a sprint, all assignees of those tasks should be able to see the sprint and its board. Currently `list_sprints` returns all sprints regardless of relevance to the user.

**BL3.** Daily standup view — show each team member's yesterday completed, today planned, and blockers (tasks with unresolved dependencies). This is the core Scrum ceremony data that's already in the DB but not surfaced.

**BL4.** Sprint goal tracking — the sprint has a `goal` field but it's just text. Add a "Goal met?" checkbox on sprint completion and include it in the retrospective export.

**BL5.** Task status transitions — currently any status can be set to any other status. Enforce valid Scrum transitions: `backlog → active → completed`, `active → blocked` (when dependencies unmet), with override for sprint owners.

**BL6.** Sprint velocity auto-calculation — after completing a sprint, automatically calculate and store velocity (points/hours completed). Show velocity trend across last N sprints on the sprint list page.

**BL7.** Blocked task detection — tasks with unresolved dependencies (dependency task not completed) should automatically show as "blocked" in the sprint board. Currently dependencies exist but don't affect the board view.

**BL8.** Sprint scope change tracking — when tasks are added/removed from an active sprint, log it in the audit trail with the sprint context. Currently only task CRUD is audited, not sprint membership changes.

**BL9.** Team workload view — show hours/points assigned per team member across active sprints. Helps sprint planning by showing who's overloaded.

**BL10.** Sprint retrospective workflow — after completing a sprint, prompt for retro notes with structured sections (what went well, what didn't, action items). Currently it's a free-text field.

---

## Business Logic — Timer & Productivity (5)

**BL11.** Focus time report — weekly/monthly summary showing: total focus hours, average daily focus, most productive day/time, longest streak. The raw data exists in sessions but isn't aggregated.

**BL12.** Task time estimate vs actual — on task completion, show the variance between estimated and actual pomodoros/hours. Surface this in sprint retrospective to improve estimation accuracy.

**BL13.** Break compliance tracking — track whether users actually take breaks or skip them. Show break-to-work ratio in the personal dashboard. Burnout prevention signal.

**BL14.** Session notes prompt — after a work session completes, show a quick note input (already supported via `PUT /api/sessions/{id}/note` but no frontend prompt). Captures what was accomplished while it's fresh.

**BL15.** Daily goal progress notification — when a user reaches their daily goal (e.g., 8 pomodoros), show a celebration/completion message. The `daily_completed` vs `daily_goal` data exists but isn't surfaced.

---

## Business Logic — Estimation & Planning (5)

**BL16.** Estimation accuracy report — after a sprint, compare room estimates vs actual time spent per task. Shows which tasks were under/over-estimated and by how much.

**BL17.** Planning poker history — when starting a new estimation round for a similar task, show the team's previous estimates for comparable tasks (same project/labels).

**BL18.** Sprint capacity planning — when adding tasks to a sprint, show running total of estimated hours vs sprint capacity. Warn when capacity is exceeded.

**BL19.** Unestimated task warning — in sprint planning, highlight tasks that have no estimates (0 hours, 0 points). These should be estimated before the sprint starts.

**BL20.** Estimation confidence — after voting in a room, if consensus is low (high variance), flag the task for discussion. Currently votes are revealed but variance isn't highlighted.

---

## Business Logic — Notifications & Awareness (3)

**BL21.** Task assignment notification — when a user is assigned to a task, they should see it in their dashboard/notification area. Currently assignments are silent.

**BL22.** Sprint event notifications — notify team members when: sprint starts, sprint is about to end (1 day before end_date), sprint completes. Uses the existing notification_prefs system.

**BL23.** Comment mention notifications — when a comment contains `@username`, notify that user. The comment system exists but has no mention detection.

---

## Summary

| Category                        | Count |
|---------------------------------|-------|
| Confirmed Bugs                  | 7     |
| Sprint & Scrum Workflow         | 10    |
| Timer & Productivity            | 5     |
| Estimation & Planning           | 5     |
| Notifications & Awareness       | 3     |
| **Total**                       | **30** |
