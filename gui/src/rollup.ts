import type { TaskDetail } from "./store/api";

export interface RollupStats {
  ownEstHours: number;
  ownSpentHours: number;
  ownEstPoints: number;
  ownRemPoints: number;
  ownSessionSecs: number;
  childEstHours: number;
  childSpentHours: number;
  childEstPoints: number;
  childRemPoints: number;
  childSessionSecs: number;
  totalEstHours: number;
  totalSpentHours: number;
  totalEstPoints: number;
  totalRemPoints: number;
  totalSessionSecs: number;
  progressHours: number | null;
  progressPoints: number | null;
}

export function computeRollup(d: TaskDetail, hoursMap: Map<number, number>): RollupStats {
  const ownEstHours = d.task.estimated_hours || 0;
  const ownSpentHours = hoursMap.get(d.task.id) ?? 0;
  const ownEstPoints = d.task.estimated || 0;
  const ownRemPoints = d.task.remaining_points || 0;
  const ownSessionSecs = d.sessions.reduce((a, s) => a + (s.duration_s ?? 0), 0);

  let childEstHours = 0, childSpentHours = 0, childEstPoints = 0, childRemPoints = 0, childSessionSecs = 0;
  for (const ch of d.children) {
    const cr = computeRollup(ch, hoursMap);
    childEstHours += cr.totalEstHours;
    childSpentHours += cr.totalSpentHours;
    childEstPoints += cr.totalEstPoints;
    childRemPoints += cr.totalRemPoints;
    childSessionSecs += cr.totalSessionSecs;
  }

  const totalEstHours = ownEstHours + childEstHours;
  const totalSpentHours = ownSpentHours + childSpentHours;
  const totalEstPoints = ownEstPoints + childEstPoints;
  const totalRemPoints = ownRemPoints + childRemPoints;
  const totalSessionSecs = ownSessionSecs + childSessionSecs;

  const progressHours = totalEstHours > 0 ? Math.min(100, Math.round((totalSpentHours / totalEstHours) * 100)) : null;
  const progressPoints = totalEstPoints > 0 ? Math.min(100, Math.round(((totalEstPoints - totalRemPoints) / totalEstPoints) * 100)) : null;

  return {
    ownEstHours, ownSpentHours, ownEstPoints, ownRemPoints, ownSessionSecs,
    childEstHours, childSpentHours, childEstPoints, childRemPoints, childSessionSecs,
    totalEstHours, totalSpentHours, totalEstPoints, totalRemPoints, totalSessionSecs,
    progressHours, progressPoints,
  };
}
