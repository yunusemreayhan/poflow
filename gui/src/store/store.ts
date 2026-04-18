import { create } from "zustand";
import { createAuthSlice, type AuthSlice } from "./authSlice";
import { createTimerSlice, type TimerSlice } from "./timerSlice";
import { createTasksSlice, type TasksSlice } from "./tasksSlice";
import { createUiSlice, type UiSlice } from "./uiSlice";
export type { SavedServer } from "./authSlice";

type Store = AuthSlice & TimerSlice & TasksSlice & UiSlice;

export const useStore = create<Store>()((...a) => ({
  ...createAuthSlice(...a as any),
  ...createTimerSlice(...a as any),
  ...createTasksSlice(...a as any),
  ...createUiSlice(...a as any),
}));
