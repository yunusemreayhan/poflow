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

  // Empty states
  noSprintsYet: string;
  noTeamsYet: string;
  noTemplatesYet: string;
  noWebhooksYet: string;
  noActivityRecorded: string;
  noRootTasks: string;
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
  api: "API",
  keyboardShortcuts: "Keyboard Shortcuts",
  focusSearch: "Focus search",
  toggleShortcuts: "Toggle this panel",
  renameTask: "Rename task",
  saveEdit: "Save inline edit",
  cancelEdit: "Cancel inline edit",
  contextMenu: "Context menu",

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
  noSprintsYet: "No sprints yet",
  noTeamsYet: "No teams yet",
  noTemplatesYet: "No templates yet",
  noWebhooksYet: "No webhooks configured",
  noActivityRecorded: "No activity recorded",
  noRootTasks: "No root tasks — team sees nothing",
};

// Available locales — add new languages here
const tr: Locale = {
  appName: "Pomodoro",
  logout: "Çıkış",
  settings: "Ayarlar",
  timer: "Zamanlayıcı",
  tasks: "Görevler",
  history: "Geçmiş",
  sprints: "Sprintler",
  rooms: "Odalar",
  skipToContent: "İçeriğe geç",
  api: "API",
  keyboardShortcuts: "Klavye Kısayolları",
  focusSearch: "Aramaya odaklan",
  toggleShortcuts: "Bu paneli aç/kapat",
  renameTask: "Görevi yeniden adlandır",
  saveEdit: "Düzenlemeyi kaydet",
  cancelEdit: "Düzenlemeyi iptal et",
  contextMenu: "Bağlam menüsü",
  login: "Giriş",
  register: "Kayıt",
  username: "Kullanıcı adı",
  password: "Şifre",
  loginButton: "Giriş Yap",
  registerButton: "Hesap Oluştur",
  switchToRegister: "Hesabınız yok mu?",
  switchToLogin: "Zaten hesabınız var mı?",
  serverUrl: "Sunucu URL",
  start: "Başla",
  pause: "Duraklat",
  resume: "Devam",
  stop: "Durdur",
  skip: "Atla",
  work: "Çalışma",
  shortBreak: "Kısa Mola",
  longBreak: "Uzun Mola",
  idle: "Hazır",
  sessionsToday: "Bugünkü oturumlar",
  dailyGoal: "Günlük hedef",
  searchTasks: "Görev ara (regex)... (/ tuşu)",
  addTask: "Görev ekle",
  addSubtask: "Alt görev ekle",
  deleteTask: "Sil",
  editTitle: "Yeniden adlandır",
  editDescription: "Açıklamayı düzenle",
  viewDetails: "Detayları gör",
  logTime: "Süre kaydet",
  comment: "Yorum",
  startTimer: "Zamanlayıcıyı başlat",
  moveUp: "Yukarı taşı",
  moveDown: "Aşağı taşı",
  status: "Durum",
  priority: "Öncelik",
  estimated: "Tahmini",
  actual: "Gerçek",
  completed: "Tamamlandı",
  active: "Aktif",
  backlog: "Yapılacak",
  all: "Tümü",
  noTasks: "Henüz görev yok",
  confirmDelete: "Bu görevi ve tüm alt görevleri sil?",
  selected: "seçili",
  markDone: "✓ Tamamla",
  markActive: "↺ Aktif yap",
  clearSearch: "Aramayı temizle",
  results: "sonuç",
  labels: "Etiketler",
  addLabel: "Ekle",
  labelName: "Etiket adı",
  noLabels: "Etiket yok",
  createInSettings: "Ayarlardan oluştur",
  dependsOn: "Bağımlı:",
  addDependency: "+ Bağımlılık ekle",
  none: "Yok",
  addRecurrence: "Tekrar ekle",
  daily: "günlük",
  weekly: "haftalık",
  biweekly: "iki haftalık",
  monthly: "aylık",
  nextDue: "sonraki",
  edit: "düzenle",
  remove: "kaldır",
  save: "Kaydet",
  cancel: "İptal",
  createSprint: "Yeni Sprint",
  startSprint: "Sprint Başlat",
  completeSprint: "Sprint Tamamla",
  sprintGoal: "Sprint hedefi",
  burndown: "Burndown",
  velocity: "Hız",
  board: "Pano",
  planning: "Planlama",
  createRoom: "Yeni Oda",
  joinRoom: "Katıl",
  leaveRoom: "Ayrıl",
  startVoting: "Oylamayı Başlat",
  castVote: "Oy Ver",
  revealVotes: "Oyları Göster",
  acceptEstimate: "Kabul Et",
  workDuration: "Çalışma süresi (dk)",
  shortBreakDuration: "Kısa mola (dk)",
  longBreakDuration: "Uzun mola (dk)",
  longBreakInterval: "Uzun mola aralığı",
  autoStartBreaks: "Molaları otomatik başlat",
  autoStartWork: "Çalışmayı otomatik başlat",
  desktopNotifications: "Masaüstü bildirimleri",
  soundNotifications: "Ses bildirimleri",
  theme: "Tema",
  darkTheme: "Koyu",
  lightTheme: "Açık",
  savedChanges: "Ayarlar kaydedildi",
  teams: "Takımlar",
  newTeam: "+ Yeni Takım",
  members: "Üyeler",
  auditLog: "Denetim Günlüğü",
  action: "İşlem",
  entity: "Varlık",
  exportTasks: "Görevleri Dışa Aktar",
  exportSessions: "Oturumları Dışa Aktar",
  loading: "Yükleniyor...",
  error: "Hata",
  success: "Başarılı",
  confirm: "Onayla",
  delete: "Sil",
  close: "Kapat",
  hours: "saat",
  points: "puan",
  description: "Açıklama",
  created: "Oluşturulma",
  updated: "Güncellenme",
  noSprintsYet: "Henüz sprint yok",
  noTeamsYet: "Henüz takım yok",
  noTemplatesYet: "Henüz şablon yok",
  noWebhooksYet: "Webhook yapılandırılmamış",
  noActivityRecorded: "Etkinlik kaydedilmemiş",
  noRootTasks: "Kök görev yok — takım hiçbir şey görmez",
};

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

export const useI18n = create<I18nState>((set) => ({
  locale: getStorage("locale", "en"),
  t: locales[getStorage("locale", "en")] || en,
  setLocale: (locale: string) => {
    setStorage("locale", locale);
    set({ locale, t: locales[locale] || en });
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
