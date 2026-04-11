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
  // Sprints (I2)
  sprintName: "Sprint name",
  project: "Project",
  sprintDuration: "Sprint Duration",
  todo: "Todo",
  inProgress: "In Progress",
  done: "Done",
  summary: "Summary",
  retroNotes: "Retro Notes",
  addRetroNotes: "Add retrospective notes...",
  useTemplate: "Use template",
  searchRootTasks: "Search root tasks...",
  startThisSprint: "Start this sprint?",
  completeThisSprint: "Complete this sprint?",
  noSprintTasks: "No tasks in sprint",
  // Estimation rooms (I3)
  pickEstimate: "Pick your estimate",
  revealCards: "Reveal Cards",
  consensus: "Consensus",
  noConsensus: "No consensus",
  average: "Average",
  waitingForAdmin: "Waiting for admin to select a task...",
  selectTask: "Select a task from the Tasks tab to start voting",
  closeRoom: "Close room",
  // History (I5)
  totalSessions: "Total Sessions",
  focusHours: "Focus Hours",
  currentStreak: "Current Streak",
  recentSessions: "Recent Sessions",
  exportCsv: "↓ Export CSV",
  allUsers: "All users",
  thisWeek: "This Week",
  activityHeatmap: "Activity heatmap",
  // Settings (I7)
  timerDurations: "Timer Durations",
  automation: "Automation",
  notifications: "Notifications",
  goals: "Goals",
  account: "Account",
  server: "Server",
  estimationMode: "Estimation Mode",
  dailyGoalSessions: "Daily Goal (sessions)",
  backendUrl: "Backend URL",
  newPassword: "New Password",
  profileUpdated: "Profile updated!",
  saveSettings: "Save Settings",
  userManagement: "User Management",
  promoteToRoot: "Promote to root",
  demoteToUser: "Demote to user",
  deleteUser: "Delete user",
  // Auth (I8)
  createAccount: "Create your account",
  signIn: "Sign in to continue",
  firstUserAdmin: "First user becomes admin",
  // SprintViews (I6)
  logBurn: "Log Burn",
  noBurnsLogged: "No burns logged",
  velocityTrend: "Velocity Trend",
  noMatchingTasks: "No matching tasks",
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
  sprintName: "Sprint adı",
  project: "Proje",
  sprintDuration: "Sprint Süresi",
  todo: "Yapılacak",
  inProgress: "Devam Eden",
  done: "Tamamlandı",
  summary: "Özet",
  retroNotes: "Retro Notları",
  addRetroNotes: "Retrospektif notları ekle...",
  useTemplate: "Şablon kullan",
  searchRootTasks: "Kök görev ara...",
  startThisSprint: "Bu sprint başlatılsın mı?",
  completeThisSprint: "Bu sprint tamamlansın mı?",
  noSprintTasks: "Sprintte görev yok",
  pickEstimate: "Tahmininizi seçin",
  revealCards: "Kartları Göster",
  consensus: "Uzlaşma",
  noConsensus: "Uzlaşma yok",
  average: "Ortalama",
  waitingForAdmin: "Yöneticinin görev seçmesi bekleniyor...",
  selectTask: "Oylama başlatmak için Görevler sekmesinden bir görev seçin",
  closeRoom: "Odayı kapat",
  totalSessions: "Toplam Oturum",
  focusHours: "Odaklanma Saati",
  currentStreak: "Mevcut Seri",
  recentSessions: "Son Oturumlar",
  exportCsv: "↓ CSV İndir",
  allUsers: "Tüm kullanıcılar",
  thisWeek: "Bu Hafta",
  activityHeatmap: "Etkinlik haritası",
  timerDurations: "Zamanlayıcı Süreleri",
  automation: "Otomasyon",
  notifications: "Bildirimler",
  goals: "Hedefler",
  account: "Hesap",
  server: "Sunucu",
  estimationMode: "Tahmin Modu",
  dailyGoalSessions: "Günlük Hedef (oturum)",
  backendUrl: "Sunucu URL",
  newPassword: "Yeni Şifre",
  profileUpdated: "Profil güncellendi!",
  saveSettings: "Ayarları Kaydet",
  userManagement: "Kullanıcı Yönetimi",
  promoteToRoot: "Yönetici yap",
  demoteToUser: "Kullanıcıya düşür",
  deleteUser: "Kullanıcıyı sil",
  createAccount: "Hesabınızı oluşturun",
  signIn: "Devam etmek için giriş yapın",
  firstUserAdmin: "İlk kullanıcı yönetici olur",
  logBurn: "Burn Kaydet",
  noBurnsLogged: "Burn kaydı yok",
  velocityTrend: "Hız Trendi",
  noMatchingTasks: "Eşleşen görev yok",
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
