# Deep Codebase Analysis — 2026-04-19

Full audit of schema (23 migrations, ~25 tables), all route handlers (165 routes), engine, auth, frontend store, 37 flow documents, and feature backlog.

---

## Critical Business Gap: "Project" Is a Phantom Entity

**This is the single most damaging architectural flaw for a Jira-replacement product.**

`project` is a free-text `TEXT` field on `tasks`, `sprints`, and `rooms`. There is no `projects` table.

### Consequences

1. **No project-level permissions.** Cannot scope visibility per project. RBAC is global (root/admin/user). Every user sees every task. The only scoping mechanism is Teams (manual root-task mapping), not project-based.

2. **No project settings.** No per-project default workflow, custom fields, or labels. Everything is global.

3. **No project consistency.** "MyProject", "my-project", "My Project" are all different. No normalization, no autocomplete enforcement, no rename propagation.

4. **No project dashboard.** Can filter by project string, but no project-level burndown, velocity, or member list.

5. **No project lifecycle.** No way to archive, set a lead, or close a project.

### Recommendation

Create a `projects` table (id, name, description, lead_user_id, status, created_at). Migrate existing `project` text fields to FK references. Add project-level permissions.

---

## Schema & Table Flaws

### 1. No Workflow Transition Rules for Custom Statuses

`custom_statuses` has `name`, `color`, `category` (todo/in_progress/done), `sort_order`. ~~But there are **no transition rules**. Any task can jump from any status to any other.~~ **FIXED (Sprint 15):** `status_transitions` table added with per-project and global rules. Enforced on single and bulk task status updates.

### 2. Missing Tables for "Implemented" Features

- **F26 (Saved filters/views):** Marked ✅ in backlog, but `saved_views` table does not exist in the codebase. Views are likely frontend-only (localStorage) — not shared across devices or users.
- **F3 (Smart scheduling):** Marked ✅, but `focus_patterns` table does not exist. Endpoint computes on-the-fly from sessions, which works, but the backlog spec called for a dedicated table.

### 3. TEXT Dates, No Timezone Handling

All dates stored as `TEXT` in `%Y-%m-%dT%H:%M:%S%.3f` (UTC). Issues:
- No timezone column on users — server assumes UTC, client converts locally, but server-side scheduled reports, due date reminders, and iCal feeds can't know user timezone.
- `due_date` is `YYYY-MM-DD` (date only) while `created_at`/`updated_at` are datetime strings. Inconsistent.
- Date comparisons are string comparisons. Works for ISO format but fragile.

### 4. `tasks.user_id` Conflates Creator and Owner

`user_id` means both "who created this" and "who owns this." No way to transfer ownership without changing the creator. Managers create tasks and assign them, but remain "owner" forever. `task_assignees` exists but assignees have limited permissions.

### 5. No `updated_by` Tracking

Tasks have `updated_at` but no `updated_by`. Can't tell who modified a task without querying the audit log separately.

### 6. Sprint-Task Relationship Is Flat

`sprint_tasks` is a simple many-to-many with no:
- `added_order` for sprint-specific ordering
- `sprint_status_override` (task might be "in_progress" globally but "blocked" in this sprint)
- `sprint_points` (task estimated at 5 globally but scoped to 3 in this sprint)

---

## Flow Gaps

### 1. No Workflow Enforcement

Custom statuses exist but there's no state machine. No "required fields before moving to Done" (e.g., must have hours logged). No approval gates. Dependencies exist but don't prevent status changes on blocked tasks.

### 2. No Project-Level Views

Frontend has team filtering but no project filtering in the sidebar. No dedicated project view with its own burndown, velocity, or member list.

### 3. Notifications Are Fire-and-Forget

`notifications` table stores notifications, but no email delivery. `email.rs` exists behind a feature flag (`#[cfg(feature = "email")]`) and isn't wired up. Due date reminders are desktop-only (`notify-rust`). Miss them if not at your computer.

### 4. No Comment/Watcher Notifications

When someone comments on a task, there's no notification to watchers. `task_watchers` table exists but watcher-triggered notifications are not integrated into the comment creation flow.

### 5. Automation Rules Are a Skeleton

`automation_rules` table exists with CRUD endpoints. ~~`automation.rs` is only 3.5KB. Rules are not evaluated on task events — the trigger/condition/action system is defined but not wired into the task update flow.~~ **FIXED (Sprint 15):** Automation rules now fire on 5 trigger events: `task.status_changed`, `task.all_subtasks_done`, `task.created`, `task.assigned`, `task.priority_changed`. All wired into the respective route handlers.

---

## Internal Workings Issues

### 1. `/api/tasks/full` Loads ALL Tasks

The bulk endpoint returns every non-deleted task plus all sprint mappings, burn totals, assignees, and labels. No pagination, no project scoping, no lazy loading. ETag caching avoids redundant transfers but the server computes the full response every time. Scaling wall at 50+ users / 10,000+ tasks.

### 2. Timer State Is In-Memory Only

`Engine.states` is a `HashMap<i64, EngineState>` in memory. Daemon restart loses all running timers. Recovery logic marks interrupted sessions in DB, but timers don't resume — they just clean up. Users mid-poflow lose their session on restart.

### 3. Webhook Delivery Has No Dead Letter Queue

`webhook.rs` has retry logic (3 attempts), but failed webhooks are silently dropped. No `webhook_deliveries` table, no visibility into failures, no manual retry. Reliability gap for external integrations.

### 4. Global OnceLock Statics in Auth

`SECRET`, `BLOCKLIST`, `AUTH_POOL` are global `OnceLock` statics. Makes testing harder (can't reset between runs), prevents multiple daemon instances in same process. `user_auth_cache` was moved to per-Engine, but token blocklist is still global.

### 5. No Connection Pool Monitoring

Pool set to `max_connections(8)` with 10s busy timeout. No pool metrics, no monitoring for contention under load.

---

## What's Done Well

- **Auth:** JWT with refresh tokens, CSRF, token blocklist, password-change invalidation, rate limiting (10/min auth, 200/min API).
- **Security headers:** CSP, HSTS, X-Frame-Options, X-Content-Type-Options, Permissions-Policy.
- **Hierarchical tasks:** Recursive CTEs for parent/child traversal.
- **FTS5 search** with graceful LIKE fallback.
- **Soft delete** with trash/restore UX.
- **Timer engine:** Two-phase tick (lock for state, unlock for DB) is a smart concurrency pattern.
- **Test coverage:** 481 backend + 206 frontend + 887 E2E tests.
- **Offline support:** IndexedDB cache, mutation queue, conflict detection via `expected_updated_at`.

---

## Priority Recommendations

| # | Item | Effort | Impact |
|---|---|---|---|
| 1 | ~~Create `projects` table, migrate text fields to FK~~ | ~~High~~ | ~~Critical~~ — **DONE (Sprint 14)** |
| 2 | ~~Add workflow transition rules (`status_transitions` table)~~ | ~~Medium~~ | ~~High~~ — **DONE (Sprint 15)** |
| 3 | Wire watcher notifications into comment/status-change flows | Low | High — basic collaboration expectation |
| 4 | Add `updated_by` to tasks | Low | Medium — traceability |
| 5 | Paginate or scope `/api/tasks/full` | Medium | High — scaling prerequisite |
| 6 | Create `saved_views` table for F26 | Low | Medium — cross-device view persistence |
| 7 | ~~Wire automation rules into task event flow~~ | ~~Medium~~ | ~~Medium~~ — **DONE (Sprint 15)** |
| 8 | Add user timezone to `users` table | Low | Medium — correct scheduled reports and reminders |
| 9 | Add `webhook_deliveries` table for observability | Low | Medium — integration reliability |
| 10 | Persist timer state to DB for restart resilience | Medium | Low — rare edge case but poor UX when it hits |
