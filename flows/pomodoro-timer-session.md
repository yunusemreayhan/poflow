# Flow: Poflow Timer Session (Core)

## Actor
Any authenticated user.

## Timer Phases
```
Idle → Work → ShortBreak → Work → ShortBreak → ... → Work → LongBreak → Work → ...
```
Long break triggers every `long_break_interval` work sessions (default: 4).

## Per-User State
Each user has an independent `EngineState`:
- `phase`: Idle / Work / ShortBreak / LongBreak
- `status`: Idle / Running / Paused
- `elapsed_s` / `duration_s`: progress tracking
- `session_count`: work sessions completed in current cycle
- `daily_completed`: work sessions completed today
- `current_task_id`: optional linked task
- `current_session_id`: DB session record

## Steps

### Start (`POST /api/timer/start`)
1. Load per-user config (work duration, break durations, etc.).
2. If a timer is already running → stop it (mark session as "interrupted").
3. Determine phase: `Work` (default), `ShortBreak`, or `LongBreak` (from `req.phase`).
4. Calculate duration from config (per-task override if set via `work_duration_minutes`).
5. Create DB session record (`sessions` table).
6. Set state to Running, broadcast via SSE.

### Pause (`POST /api/timer/pause`)
- Sets status to `Paused`. Timer stops counting. No DB write.

### Resume (`POST /api/timer/resume`)
- Sets status back to `Running`. Timer resumes from `elapsed_s`.

### Stop (`POST /api/timer/stop`)
- Ends current session as "interrupted" in DB.
- Resets to Idle, preserving `session_count`, `daily_completed`, `daily_goal`.

### Skip (`POST /api/timer/skip`)
- Ends current session as "skipped" in DB.
- Advances to next phase (Work→Break or Break→Work).
- Sets status to Idle (user must manually start next phase).

### Tick (Background, every 1 second)
1. For each user with `status == Running`: increment `elapsed_s`.
2. If `elapsed_s >= duration_s` → session complete:
   - End session as "completed" in DB.
   - If was Work: increment `session_count` and `daily_completed`.
   - If was Work + linked task: increment `tasks.actual`, log burn entry.
   - Advance to next phase.
   - If `auto_start_breaks`/`auto_start_work` → auto-start next session.
   - Desktop notification sent (if enabled in user config).
3. Broadcast state via SSE `watch` channel.

### Poll (`GET /api/timer`)
- Returns current state for the authenticated user.
- Refreshes `daily_completed` from DB (not cached).

### Active Timers (`GET /api/timer/active`)
- Returns all users with running/paused timers.
- Shows username, phase, status, task title, elapsed/duration.
- Any authenticated user can see all active timers.

## Authorization
- Each user can only control their own timer.
- `claims.user_id` is used for all operations — no way to control another user's timer.
- Active timers are visible to everyone (team transparency).

## Per-User Config
Users can override global timer settings via `PUT /api/config`:
- `work_duration_min` (1–240, default 25)
- `short_break_min` (1–60, default 5)
- `long_break_min` (1–120, default 15)
- `long_break_interval` (1–20, default 4)
- `auto_start_breaks` / `auto_start_work`
- `daily_goal` (0–50, default 8)

Root users also update the global config file.

## Midnight Reset
Background task detects date change → resets `daily_completed` to 0 for all users.
