# FEATURES BACKLOG — pomodoroLinux

New feature proposals to make pomodoroLinux a best-in-class Pomodoro + project management tool. Organized by category, prioritized by impact within each section.

Existing feature inventory: 154 API endpoints covering timer, tasks (hierarchical, labels, dependencies, recurrence, attachments, templates, watchers, FTS), sprints (board, burndown, velocity, carry-over, snapshots), planning poker rooms (WebSocket, auto-advance), teams, epics, notifications, webhooks, audit log, import/export (CSV/JSON), backup/restore, per-user config, multi-server support.

---

## 1. Calendar & Time Intelligence

- [ ] **F1. Calendar view.** Frontend calendar component showing tasks by due_date, sessions by date, and sprint date ranges. Week/month toggle. Drag-to-reschedule due dates.
  - Backend: No changes needed — all data available via existing endpoints.
  - Frontend: New `Calendar.tsx` component using a lightweight grid (no heavy lib).

- [ ] **F2. Focus time heatmap.** GitHub-style contribution heatmap showing daily focus minutes over the past year. Already have `/api/stats?days=365` — just needs a frontend visualization.

- [ ] **F3. Smart scheduling suggestions.** Backend endpoint `GET /api/suggestions/schedule` that analyzes historical session patterns (peak productivity hours, average session count per day) and suggests optimal time slots for upcoming tasks based on estimated hours and due dates.
  - New table: `focus_patterns` (user_id, hour_of_day, avg_sessions, avg_focus_min).
  - Cron job or on-demand aggregation from sessions table.

- [ ] **F4. iCal feed.** `GET /api/export/ical` — read-only iCal (.ics) feed of tasks with due dates and sprint date ranges. Allows subscribing from Google Calendar, Outlook, Apple Calendar. Token-authenticated via query param (like SSE tickets).

## 2. Advanced Analytics & Insights

- [ ] **F5. Productivity trends dashboard.** Weekly/monthly aggregated view: focus hours trend, sessions trend, completion rate, average session length, interruption rate. Compare current week vs previous week.
  - Backend: `GET /api/analytics/trends?period=week&count=8` — returns array of period summaries.
  - Frontend: Line charts using lightweight SVG (no chart library).

- [ ] **F6. Task estimation accuracy report.** Compare estimated vs actual pomodoros/hours across completed tasks. Show accuracy percentage, over/under-estimation patterns, per-project breakdown.
  - Backend: `GET /api/analytics/estimation-accuracy?project=X` — computed from tasks where status=completed and estimated>0.

- [ ] **F7. Sprint retrospective analytics.** Auto-generated sprint report with: velocity trend, scope change (tasks added/removed mid-sprint), carry-over rate, per-member contribution breakdown, burndown ideal vs actual comparison.
  - Backend: Most data exists in sprint_tasks + burn_log + snapshots. New endpoint `GET /api/sprints/{id}/retro-report` to aggregate.

- [ ] **F8. Personal focus score.** Daily/weekly score (0-100) based on: sessions completed vs goal, interruption rate, consistency (streak), estimation accuracy. Gamification-lite without being annoying.
  - Backend: `GET /api/analytics/focus-score` — computed from sessions + config.

## 3. Collaboration & Communication

- [ ] **F9. Activity feed with @mentions.** Real-time activity stream showing task updates, comments, sprint changes across the team. Already have audit log + @mention parsing in comments — extend to a dedicated feed endpoint with filtering.
  - Backend: `GET /api/feed?since=<iso>&types=comment,status_change,assignment` — union of audit + comments + notifications.

- [ ] **F10. Task chat / threaded comments.** Allow reply-to on comments (parent_comment_id). Show threaded discussions on task detail view. Already have comments — just add threading.
  - Backend: Add `parent_id` column to comments table. Migration v11.
  - Frontend: Indent replies in CommentSection.

- [ ] **F11. Shared timer sessions.** "Pair programming" mode — two users can join the same timer session on the same task. Both get credit. Useful for mob/pair work.
  - Backend: New table `session_participants` (session_id, user_id). Modify burn logging to credit all participants.

- [ ] **F12. Presence indicators.** Show which users are currently online (have active SSE connections) and what they're working on. Already have `/api/timer/active` — extend with "last seen" for offline users.
  - Backend: Track SSE connection count per user. `GET /api/users/presence`.

## 4. Integrations

- [ ] **F13. GitHub/GitLab integration.** Link commits/PRs to tasks via branch naming convention (`task-123-description`) or commit message tags (`#123`). Show linked commits on task detail.
  - Backend: Webhook receiver `POST /api/integrations/github` that parses push events and links to tasks.
  - New table: `task_links` (task_id, link_type, url, title, created_at).

- [ ] **F14. Slack/Discord notifications.** Configurable bot that posts to a channel when: sprint starts/completes, daily standup summary, task assigned. Use existing webhook infrastructure — just add a "Slack webhook URL" integration type.
  - Backend: New integration type in webhooks with Slack-formatted payloads.

- [ ] **F15. REST API client SDK.** Auto-generated TypeScript and Python client libraries from the existing utoipa/OpenAPI spec. Publish as npm/pip packages.
  - Tooling: `cargo run --bin generate-openapi > openapi.json`, then use openapi-generator.

## 5. Offline & Mobile

- [ ] **F16. Offline mode with sync.** Service worker + IndexedDB cache for the Tauri web view. Queue mutations when offline, sync when reconnected. Conflict resolution via `expected_updated_at` (already implemented).
  - Frontend: Service worker registration, IndexedDB store, mutation queue, sync on reconnect.

- [ ] **F17. Progressive Web App (PWA).** Add manifest.json, service worker, and installability for the web version. Works alongside Tauri for desktop.

- [ ] **F18. Mobile-responsive layout.** The current 72px sidebar doesn't work on mobile. Add a bottom tab bar for <768px screens, collapsible sidebar for tablets.
  - Frontend: CSS media queries + conditional rendering in App.tsx.

## 6. Automation & Workflows

- [ ] **F19. Custom automation rules.** User-defined triggers: "When task status changes to completed → auto-archive after 7 days", "When all subtasks completed → mark parent as completed", "When due date is tomorrow → set priority to 5".
  - Backend: New table `automation_rules` (user_id, trigger, condition_json, action_json, enabled). Evaluated on task/sprint events.
  - Start with 3-5 built-in rule templates, not a full scripting engine.

- [ ] **F20. Scheduled reports.** Weekly email/webhook digest: tasks completed, focus hours, sprint progress, upcoming due dates. Cron-based.
  - Backend: Background task that runs daily/weekly, generates report JSON, dispatches via webhook.

- [ ] **F21. Auto-prioritization.** Suggest priority adjustments based on: approaching due date, dependency chain depth, sprint deadline proximity, time since last update.
  - Backend: `GET /api/suggestions/priorities` — returns list of (task_id, suggested_priority, reason).

## 7. Gamification & Motivation

- [ ] **F22. Streaks & achievements.** Track consecutive days with ≥1 completed session. Award badges: "7-day streak", "100 sessions", "First sprint completed", "Estimation accuracy >80%".
  - Backend: New table `achievements` (user_id, achievement_type, unlocked_at). Checked on session completion.
  - Frontend: Achievement toast + profile badge display.

- [ ] **F23. Focus leaderboard.** Optional team leaderboard showing weekly focus hours, sessions, tasks completed. Opt-in per user (privacy setting).
  - Backend: `GET /api/leaderboard?period=week` — aggregated from sessions. Respects opt-in flag.

## 8. Advanced Task Management

- [ ] **F24. Kanban board view.** Drag-and-drop board with customizable columns (not just sprint board — a general task board). Columns map to statuses. Swimlanes by project or assignee.
  - Frontend: New `KanbanBoard.tsx` with drag-and-drop (use native HTML5 DnD, no library).

- [ ] **F25. Task time estimates with confidence intervals.** Instead of single-point estimates, allow optimistic/pessimistic/likely (PERT). Calculate expected duration and risk.
  - Backend: Add `estimate_optimistic`, `estimate_pessimistic` to tasks table. PERT formula: (O + 4M + P) / 6.

- [ ] **F26. Saved filters / custom views.** Save frequently used task filter combinations (status + project + assignee + label + priority) as named views. Quick-switch between views.
  - Backend: New table `saved_views` (user_id, name, filter_json). CRUD endpoints.
  - Frontend: View selector dropdown in TaskList.

- [ ] **F27. Task checklists.** Lightweight sub-items within a task (not full subtasks). Markdown-style `- [ ] item` in description, with toggle support.
  - Frontend-only: Parse `- [ ]` / `- [x]` in description, render as checkboxes, update description on toggle.

## 9. Developer Experience

- [ ] **F28. CLI client.** `pomodoro-cli` binary that talks to the REST API. `pomodoro start`, `pomodoro status`, `pomodoro log 2h "Fixed auth bug"`, `pomodoro tasks --status active`. Useful for terminal-centric workflows.
  - New crate: `pomodoro-cli` using clap + reqwest.

- [ ] **F29. Git hook integration.** Pre-commit hook that auto-links the current timer task to the commit message. Post-commit hook that logs time.
  - Ships as installable git hooks in the CLI package.

- [ ] **F30. VS Code extension.** Status bar showing current timer + task. Start/stop/pause from the editor. Task picker for timer start.
  - Separate repo, talks to REST API.

---

## Priority Tiers

**Tier 1 — High impact, moderate effort (do first):**
F1 (Calendar view) ✅, F2 (Focus heatmap) ✅, F5 (Productivity trends) ✅, F18 (Mobile responsive) ✅, F24 (Kanban board) ✅, F26 (Saved filters) ✅

**Tier 2 — High impact, higher effort:**
F4 (iCal feed) ✅, F6 (Estimation accuracy) ✅, F8 (Focus score) ✅, F13 (GitHub integration) ✅, F16 (Offline mode), F22 (Streaks) ✅, F28 (CLI client)

**Tier 3 — Nice to have:**
F3 (Smart scheduling), F7 (Sprint retro analytics) ✅, F9 (Activity feed), F10 (Threaded comments) ✅, F14 (Slack integration), F17 (PWA), F19 (Automation rules), F21 (Auto-prioritization) ✅, F23 (Leaderboard) ✅, F25 (PERT estimates), F27 (Checklists)

**Tier 4 — Future vision:**
F11 (Shared timer), F12 (Presence), F15 (SDK), F20 (Scheduled reports), F29 (Git hooks), F30 (VS Code extension)
