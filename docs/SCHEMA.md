# Database Schema

SQLite database with 22 tables. All timestamps are ISO 8601 strings.

## Core Tables

| Table | Description |
|---|---|
| `users` | User accounts (id, username, password_hash, role, created_at) |
| `tasks` | Task tree with soft delete (id, parent_id, user_id, title, description, project, tags, priority, estimated, actual, status, due_date, deleted_at, ...) |
| `sessions` | Poflow timer sessions (id, task_id, user_id, session_type, status, started_at, ended_at, duration_s) |
| `comments` | Task comments (id, task_id, session_id, user_id, content, created_at) |
| `task_assignees` | Many-to-many task↔user assignments |

## Sprint Tables

| Table | Description |
|---|---|
| `sprints` | Sprint definitions (id, name, goal, status, start_date, end_date) |
| `sprint_tasks` | Many-to-many sprint↔task |
| `sprint_daily_stats` | Daily snapshot of sprint progress (points, hours, task counts) |
| `sprint_root_tasks` | Root tasks for sprint board grouping |
| `burn_log` | Time/point burn entries (manual or timer-generated) |

## Estimation Room Tables

| Table | Description |
|---|---|
| `rooms` | Estimation rooms (id, name, estimation_unit, status, current_task_id) |
| `room_members` | Room membership with roles |
| `room_votes` | Individual votes per room/task/user |

## Organization Tables

| Table | Description |
|---|---|
| `teams` | Team definitions |
| `team_members` | Team membership with admin flag |
| `team_root_tasks` | Root tasks scoped to a team |
| `epic_groups` | Epic groupings |
| `epic_group_tasks` | Many-to-many epic↔task |
| `epic_snapshots` | Point-in-time epic progress snapshots |

## Metadata Tables

| Table | Description |
|---|---|
| `labels` | Shared label definitions (name, color) |
| `task_labels` | Many-to-many task↔label |
| `task_recurrence` | Recurring task patterns (daily, weekly, biweekly, monthly) |
| `task_dependencies` | Task dependency edges |
| `user_configs` | Per-user timer/theme overrides |
| `audit_log` | Action audit trail (user_id, action, entity_type, entity_id, detail) |
| `task_attachments` | File attachment metadata |
| `task_templates` | Reusable task templates (per-user) |
| `webhooks` | Webhook subscriptions (url, events, secret) |
| `token_blocklist` | Revoked JWT token hashes |

## Indexes

13 indexes on foreign keys and frequently queried columns (parent_id, status, project, user_id, task_id, sprint_id, started_at, creator_id, entity_type).
