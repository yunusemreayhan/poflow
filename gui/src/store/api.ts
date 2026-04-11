import { invoke } from "@tauri-apps/api/core";

// --- HTTP API helper ---

export async function apiCall<T = unknown>(method: string, path: string, body?: unknown): Promise<T> {
  try {
    return await invoke<T>("api_call", { method, path, body: body ?? null });
  } catch (e) {
    // Show toast for mutation failures (POST/PUT/DELETE)
    if (method !== "GET") {
      const msg = typeof e === "string" ? e : (e as Error)?.message || "Request failed";
      // Parse JSON error if possible
      try { const parsed = JSON.parse(msg); if (parsed.error) { showErrorToast(parsed.error); throw e; } } catch {}
      showErrorToast(msg);
    }
    throw e;
  }
}

function showErrorToast(msg: string) {
  // Lazy import to avoid circular dependency
  try {
    const { useStore } = require("./store");
    useStore.getState().toast(msg, "error");
  } catch {}
}

export async function setToken(token: string) {
  return invoke("set_token", { token });
}

// --- Types ---

export interface Task {
  id: number;
  parent_id: number | null;
  user_id: number;
  user: string;
  title: string;
  description: string | null;
  project: string | null;
  tags: string | null;
  priority: number;
  estimated: number;
  actual: number;
  estimated_hours: number;
  remaining_points: number;
  due_date: string | null;
  status: string;
  sort_order: number;
  created_at: string;
  updated_at: string;
}

export interface Session {
  id: number;
  task_id: number | null;
  user_id: number;
  user: string;
  session_type: string;
  status: string;
  started_at: string;
  ended_at: string | null;
  duration_s: number | null;
  notes: string | null;
  task_path?: string[];
}

export interface EngineState {
  phase: string;
  status: string;
  elapsed_s: number;
  duration_s: number;
  session_count: number;
  current_task_id: number | null;
  current_session_id: number | null;
  current_user_id: number;
  daily_completed: number;
  daily_goal: number;
}

export interface Comment {
  id: number;
  task_id: number;
  session_id: number | null;
  user_id: number;
  user: string;
  content: string;
  created_at: string;
}

export interface TaskDetail {
  task: Task;
  comments: Comment[];
  sessions: Session[];
  children: TaskDetail[];
}

export interface DayStat {
  date: string;
  completed: number;
  interrupted: number;
  total_focus_s: number;
}

export interface Config {
  work_duration_min: number;
  short_break_min: number;
  long_break_min: number;
  long_break_interval: number;
  auto_start_breaks: boolean;
  auto_start_work: boolean;
  sound_enabled: boolean;
  notification_enabled: boolean;
  daily_goal: number;
  estimation_mode: string;
  leaf_only_mode: boolean;
}

// TimeReport is now a BurnEntry (unified burn_log table)

export interface EpicGroup {
  id: number;
  name: string;
  created_by: number;
  created_at: string;
  updated_at: string;
}

export interface EpicSnapshot {
  id: number;
  group_id: number;
  date: string;
  total_tasks: number;
  done_tasks: number;
  total_points: number;
  done_points: number;
  total_hours: number;
  done_hours: number;
}

export interface EpicGroupDetail {
  group: EpicGroup;
  task_ids: number[];
  snapshots: EpicSnapshot[];
}

export interface Team {
  id: number;
  name: string;
  created_at: string;
}

export interface TeamMember {
  team_id: number;
  user_id: number;
  username: string;
  role: string;
}

export interface TeamDetail {
  team: Team;
  members: TeamMember[];
  root_task_ids: number[];
}
export type TimeReport = BurnEntry;

export interface BurnTotal {
  total_points: number;
  total_hours: number;
  count: number;
}

export interface AuthResponse {
  token: string;
  user_id: number;
  username: string;
  role: string;
}

export interface User {
  id: number;
  username: string;
  role: string;
  created_at: string;
}

// --- Room types ---

export interface Room {
  id: number;
  name: string;
  room_type: string;
  estimation_unit: string;
  project: string | null;
  creator: string;
  status: string;
  current_task_id: number | null;
  created_at: string;
}

export interface RoomMember {
  room_id: number;
  username: string;
  role: string;
  joined_at: string;
}

export interface RoomVoteView {
  username: string;
  voted: boolean;
  value: number | null;
}

export interface RoomVote {
  id: number;
  room_id: number;
  task_id: number;
  username: string;
  value: number | null;
  created_at: string;
}

export interface VoteResult {
  task_id: number;
  task_title: string;
  votes: { id: number; room_id: number; task_id: number; user_id: number; username: string; value: number | null; created_at: string }[];
  average: number;
  consensus: boolean;
}

export interface RoomState {
  room: Room;
  members: RoomMember[];
  current_task: Task | null;
  votes: RoomVoteView[];
  tasks: Task[];
  vote_history: VoteResult[];
}

// --- Sprint types ---

export interface Sprint {
  id: number;
  name: string;
  project: string | null;
  goal: string | null;
  status: string;
  start_date: string | null;
  end_date: string | null;
  created_by: string;
  created_at: string;
  updated_at: string;
}

export interface SprintTask {
  sprint_id: number;
  task_id: number;
  added_by: string;
  added_at: string;
}

export interface SprintDailyStat {
  id: number;
  sprint_id: number;
  date: string;
  total_points: number;
  done_points: number;
  total_hours: number;
  done_hours: number;
  total_tasks: number;
  done_tasks: number;
}

export interface SprintDetail {
  sprint: Sprint;
  tasks: Task[];
  stats: SprintDailyStat[];
}

export interface SprintBoard {
  todo: Task[];
  in_progress: Task[];
  done: Task[];
}

export interface TaskSprintInfo {
  task_id: number;
  sprint_id: number;
  sprint_name: string;
  sprint_status: string;
}

export interface BurnEntry {
  id: number;
  sprint_id: number | null;
  task_id: number;
  session_id: number | null;
  user_id: number;
  username: string;
  points: number;
  hours: number;
  source: string;
  note: string | null;
  cancelled: number;
  cancelled_by_id: number | null;
  cancelled_by: string | null;
  created_at: string;
}

export interface BurnSummaryEntry {
  date: string;
  username: string;
  points: number;
  hours: number;
  count: number;
}

export interface BurnTotalEntry {
  task_id: number;
  total_points: number;
  total_hours: number;
  count: number;
}

export interface TaskAssignee {
  task_id: number;
  user_id: number;
  username: string;
}
