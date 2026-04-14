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
  kanban: string;
  gantt: string;
  roadmap: string;
  calendar: string;
  skipToContent: string;
  api: string;
  keyboardShortcuts: string;
  focusSearch: string;
  toggleShortcuts: string;
  renameTask: string;
  saveEdit: string;
  cancelEdit: string;
  contextMenu: string;

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

  // Sprints (I2)
  sprintName: string;
  project: string;
  sprintDuration: string;
  todo: string;
  inProgress: string;
  done: string;
  summary: string;
  retroNotes: string;
  addRetroNotes: string;
  useTemplate: string;
  searchRootTasks: string;
  startThisSprint: string;
  completeThisSprint: string;
  noSprintTasks: string;

  // Estimation rooms (I3)
  pickEstimate: string;
  revealCards: string;
  consensus: string;
  noConsensus: string;
  average: string;
  waitingForAdmin: string;
  selectTask: string;
  closeRoom: string;

  // History (I5)
  totalSessions: string;
  focusHours: string;
  currentStreak: string;
  recentSessions: string;
  exportCsv: string;
  allUsers: string;
  thisWeek: string;
  activityHeatmap: string;

  // Settings (I7)
  timerDurations: string;
  automation: string;
  notifications: string;
  goals: string;
  account: string;
  server: string;
  estimationMode: string;
  dailyGoalSessions: string;
  backendUrl: string;
  newPassword: string;
  profileUpdated: string;
  saveSettings: string;
  userManagement: string;
  promoteToRoot: string;
  demoteToUser: string;
  deleteUser: string;

  // Auth (I8)
  createAccount: string;
  signIn: string;
  firstUserAdmin: string;

  // SprintViews (I6)
  logBurn: string;
  noBurnsLogged: string;
  velocityTrend: string;

  // Empty states
  noSprintsYet: string;
  noTeamsYet: string;
  noTemplatesYet: string;
  noWebhooksYet: string;
  noActivityRecorded: string;
  noRootTasks: string;
  noMatchingTasks: string;

  // I1-I5: Additional i18n keys
  somethingWentWrong: string;
  reload: string;
  epicBurndown: string;
  rootTasksInGroup: string;
  noRootTasksAdded: string;
  snapshotNow: string;
  deleteTeam: string;
  addMember: string;
  filterAll: string;
  filterTasks: string;
  filterUsers: string;
  filterSprints: string;
  filterRooms: string;
}

import en from "./locales/en";
import tr from "./locales/tr";

const locales: Record<string, Locale> = { en, tr };

interface I18nState {
  locale: string;
  t: Locale;
  setLocale: (locale: string) => void;
  availableLocales: () => string[];
}

function getStorage(key: string, fallback: string): string {
  try { return localStorage.getItem(key) || fallback; } catch { return fallback; }
}
function setStorage(key: string, value: string) {
  try { localStorage.setItem(key, value); } catch {}
}

function createProxiedLocale(locale: string): Locale {
  const base = locales[locale] || en;
  return new Proxy(base, { get: (target, prop: string) => (target as any)[prop] ?? (en as any)[prop] ?? prop }) as Locale;
}

export const useI18n = create<I18nState>((set) => ({
  locale: getStorage("locale", "en"),
  t: createProxiedLocale(getStorage("locale", "en")),
  setLocale: (locale: string) => {
    setStorage("locale", locale);
    set({ locale, t: createProxiedLocale(locale) });
  },
  availableLocales: () => Object.keys(locales),
}));

/** Shorthand hook */
export function useT(): Locale {
  return useI18n((s) => s.t);
}

/** Simple string interpolation: interpolate("Hello {name}", { name: "World" }) → "Hello World" */
export function interpolate(template: string, vars: Record<string, string | number>): string {
  return template.replace(/\{(\w+)\}/g, (_, key) => String(vars[key] ?? `{${key}}`));
}

/** Simple pluralization: plural(count, "session", "sessions") */
export function plural(count: number, singular: string, pluralForm: string): string {
  return count === 1 ? singular : pluralForm;
}
