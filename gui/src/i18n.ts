import { create } from "zustand";

export interface Locale {
  // App chrome
  appName: string;
  logout: string;
  settings: string;
  timer: string;
  tasks: string;
  history: string;
  sprints: string;
  rooms: string;
  skipToContent: string;

  // Auth
  login: string;
  register: string;
  username: string;
  password: string;
  loginButton: string;
  registerButton: string;
  switchToRegister: string;
  switchToLogin: string;
  serverUrl: string;

  // Timer
  start: string;
  pause: string;
  resume: string;
  stop: string;
  skip: string;
  work: string;
  shortBreak: string;
  longBreak: string;
  idle: string;
  sessionsToday: string;
  dailyGoal: string;

  // Tasks
  searchTasks: string;
  addTask: string;
  addSubtask: string;
  deleteTask: string;
  editTitle: string;
  editDescription: string;
  viewDetails: string;
  logTime: string;
  comment: string;
  startTimer: string;
  moveUp: string;
  moveDown: string;
  status: string;
  priority: string;
  estimated: string;
  actual: string;
  completed: string;
  active: string;
  backlog: string;
  all: string;
  noTasks: string;
  confirmDelete: string;
  selected: string;
  markDone: string;
  markActive: string;
  clearSearch: string;
  results: string;

  // Labels
  labels: string;
  addLabel: string;
  labelName: string;
  noLabels: string;
  createInSettings: string;

  // Dependencies
  dependsOn: string;
  addDependency: string;
  none: string;

  // Recurrence
  addRecurrence: string;
  daily: string;
  weekly: string;
  biweekly: string;
  monthly: string;
  nextDue: string;
  edit: string;
  remove: string;
  save: string;
  cancel: string;

  // Sprints
  createSprint: string;
  startSprint: string;
  completeSprint: string;
  sprintGoal: string;
  burndown: string;
  velocity: string;
  board: string;
  planning: string;

  // Rooms
  createRoom: string;
  joinRoom: string;
  leaveRoom: string;
  startVoting: string;
  castVote: string;
  revealVotes: string;
  acceptEstimate: string;

  // Settings
  workDuration: string;
  shortBreakDuration: string;
  longBreakDuration: string;
  longBreakInterval: string;
  autoStartBreaks: string;
  autoStartWork: string;
  desktopNotifications: string;
  soundNotifications: string;
  theme: string;
  darkTheme: string;
  lightTheme: string;
  savedChanges: string;

  // Teams
  teams: string;
  newTeam: string;
  members: string;

  // Audit
  auditLog: string;
  action: string;
  entity: string;

  // Export
  exportTasks: string;
  exportSessions: string;

  // Common
  loading: string;
  error: string;
  success: string;
  confirm: string;
  delete: string;
  close: string;
  hours: string;
  points: string;
  description: string;
  created: string;
  updated: string;
}

const en: Locale = {
  appName: "Pomodoro",
  logout: "Logout",
  settings: "Settings",
  timer: "Timer",
  tasks: "Tasks",
  history: "History",
  sprints: "Sprints",
  rooms: "Rooms",
  skipToContent: "Skip to content",

  login: "Login",
  register: "Register",
  username: "Username",
  password: "Password",
  loginButton: "Sign In",
  registerButton: "Create Account",
  switchToRegister: "Need an account?",
  switchToLogin: "Already have an account?",
  serverUrl: "Server URL",

  start: "Start",
  pause: "Pause",
  resume: "Resume",
  stop: "Stop",
  skip: "Skip",
  work: "Work",
  shortBreak: "Short Break",
  longBreak: "Long Break",
  idle: "Idle",
  sessionsToday: "Sessions today",
  dailyGoal: "Daily goal",

  searchTasks: "Search tasks (regex)... (press /)",
  addTask: "Add task",
  addSubtask: "Add subtask",
  deleteTask: "Delete",
  editTitle: "Rename",
  editDescription: "Edit description",
  viewDetails: "View details",
  logTime: "Log time",
  comment: "Comment",
  startTimer: "Start timer",
  moveUp: "Move up",
  moveDown: "Move down",
  status: "Status",
  priority: "Priority",
  estimated: "Estimated",
  actual: "Actual",
  completed: "Done",
  active: "Active",
  backlog: "Todo",
  all: "All",
  noTasks: "No tasks yet",
  confirmDelete: "Delete this task and all subtasks?",
  selected: "selected",
  markDone: "✓ Done",
  markActive: "↺ Active",
  clearSearch: "Clear search",
  results: "results",

  labels: "Labels",
  addLabel: "Add",
  labelName: "Label name",
  noLabels: "No labels",
  createInSettings: "create in Settings",

  dependsOn: "Depends on:",
  addDependency: "+ Add dependency",
  none: "None",

  addRecurrence: "Add recurrence",
  daily: "daily",
  weekly: "weekly",
  biweekly: "biweekly",
  monthly: "monthly",
  nextDue: "next",
  edit: "edit",
  remove: "remove",
  save: "Save",
  cancel: "Cancel",

  createSprint: "New Sprint",
  startSprint: "Start Sprint",
  completeSprint: "Complete Sprint",
  sprintGoal: "Sprint goal",
  burndown: "Burndown",
  velocity: "Velocity",
  board: "Board",
  planning: "Planning",

  createRoom: "New Room",
  joinRoom: "Join",
  leaveRoom: "Leave",
  startVoting: "Start Voting",
  castVote: "Vote",
  revealVotes: "Reveal",
  acceptEstimate: "Accept",

  workDuration: "Work duration (min)",
  shortBreakDuration: "Short break (min)",
  longBreakDuration: "Long break (min)",
  longBreakInterval: "Long break interval",
  autoStartBreaks: "Auto-start breaks",
  autoStartWork: "Auto-start work",
  desktopNotifications: "Desktop notifications",
  soundNotifications: "Sound notifications",
  theme: "Theme",
  darkTheme: "Dark",
  lightTheme: "Light",
  savedChanges: "Settings saved",

  teams: "Teams",
  newTeam: "+ New Team",
  members: "Members",

  auditLog: "Audit Log",
  action: "Action",
  entity: "Entity",

  exportTasks: "Export Tasks",
  exportSessions: "Export Sessions",

  loading: "Loading...",
  error: "Error",
  success: "Success",
  confirm: "Confirm",
  delete: "Delete",
  close: "Close",
  hours: "hours",
  points: "points",
  description: "Description",
  created: "Created",
  updated: "Updated",
};

// Available locales — add new languages here
const locales: Record<string, Locale> = { en };

interface I18nState {
  locale: string;
  t: Locale;
  setLocale: (locale: string) => void;
  availableLocales: () => string[];
}

export const useI18n = create<I18nState>((set) => ({
  locale: localStorage.getItem("locale") || "en",
  t: locales[localStorage.getItem("locale") || "en"] || en,
  setLocale: (locale: string) => {
    localStorage.setItem("locale", locale);
    set({ locale, t: locales[locale] || en });
  },
  availableLocales: () => Object.keys(locales),
}));

/** Shorthand hook */
export function useT(): Locale {
  return useI18n((s) => s.t);
}
