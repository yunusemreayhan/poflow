import { useState } from "react";
import { Timer, ListTodo, Zap, BarChart3, X } from "lucide-react";

export default function WelcomeGuide({ onDismiss }: { onDismiss: () => void }) {
  const [step, setStep] = useState(0);
  const steps = [
    { icon: Timer, title: "Focus with Pomodoro", desc: "Start a 25-minute focus session. Take short breaks between sessions. Track your daily goal." },
    { icon: ListTodo, title: "Organize Tasks", desc: "Create projects and tasks with priorities, due dates, and labels. Drag to reorder. Use checklists for sub-items." },
    { icon: Zap, title: "Plan Sprints", desc: "Group tasks into sprints with burndown charts. Use the Kanban board to track progress." },
    { icon: BarChart3, title: "Track Progress", desc: "View your focus heatmap, daily standup, and team leaderboard. Export reports as CSV." },
  ];
  const s = steps[step];
  const Icon = s.icon;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="glass p-8 rounded-2xl max-w-sm w-full mx-4 text-center relative">
        <button onClick={onDismiss} className="absolute top-3 right-3 text-white/30 hover:text-white/60"><X size={16} /></button>
        <div className="w-16 h-16 rounded-2xl bg-[var(--color-accent)]/20 flex items-center justify-center mx-auto mb-4">
          <Icon size={28} className="text-[var(--color-accent)]" />
        </div>
        <h2 className="text-lg font-semibold text-white mb-2">{s.title}</h2>
        <p className="text-sm text-white/50 mb-6">{s.desc}</p>
        <div className="flex items-center justify-between">
          <div className="flex gap-1.5">
            {steps.map((_, i) => <div key={i} className={`w-2 h-2 rounded-full ${i === step ? "bg-[var(--color-accent)]" : "bg-white/10"}`} />)}
          </div>
          {step < steps.length - 1 ? (
            <button onClick={() => setStep(step + 1)} className="px-4 py-2 rounded-lg bg-[var(--color-accent)] text-white text-sm font-medium">Next</button>
          ) : (
            <button onClick={onDismiss} className="px-4 py-2 rounded-lg bg-[var(--color-accent)] text-white text-sm font-medium">Get Started</button>
          )}
        </div>
      </div>
    </div>
  );
}
