import { useEffect, useState, useCallback } from "react";
import { MessageSquare, Zap, Clock, FileText, RefreshCw } from "lucide-react";
import { apiCall } from "../store/api";

interface FeedItem {
  type: string;
  action?: string;
  entity_type?: string;
  entity_id?: number;
  detail?: string;
  task_id?: number;
  task_title?: string;
  sprint_id?: number;
  name?: string;
  status?: string;
  content?: string;
  points?: number;
  hours?: number;
  created_at: string;
  user: string;
}

const ICONS: Record<string, typeof FileText> = {
  audit: FileText, comment: MessageSquare, sprint: Zap, burn: Clock,
};

const ACTION_LABELS: Record<string, string> = {
  create: "created", update: "updated", delete: "deleted",
  update_role: "changed role of", admin_reset_password: "reset password for",
  register: "registered",
};

function timeAgo(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  const days = Math.floor(hrs / 24);
  return `${days}d ago`;
}

function describe(item: FeedItem): string {
  if (item.type === "comment") return `commented on "${item.task_title || `#${item.task_id}`}"`;
  if (item.type === "sprint") return `sprint "${item.name}" → ${item.status}`;
  if (item.type === "burn") return `logged ${item.hours}h on "${item.task_title}"`;
  if (item.type === "audit") {
    const verb = ACTION_LABELS[item.action || ""] || item.action || "acted on";
    return `${verb} ${item.entity_type}${item.entity_id ? ` #${item.entity_id}` : ""}`;
  }
  return item.type;
}

export default function ActivityTimeline() {
  const [items, setItems] = useState<FeedItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [filter, setFilter] = useState("all");

  const load = useCallback(() => {
    setLoading(true);
    const params = filter === "all" ? "" : `&types=${filter}`;
    apiCall<FeedItem[]>("GET", `/api/feed?limit=100${params}`).then(setItems).catch(() => {}).finally(() => setLoading(false));
  }, [filter]);

  useEffect(() => { load(); }, [load]);

  const filters = [
    { id: "all", label: "All" },
    { id: "audit", label: "Actions" },
    { id: "comment", label: "Comments" },
    { id: "sprint", label: "Sprints" },
    { id: "burn", label: "Time" },
  ];

  return (
    <div className="flex flex-col gap-3 md:gap-5 p-3 md:p-8 h-full overflow-hidden">
      <div className="flex items-center gap-3">
        <span className="font-medium text-white/80 text-sm">Activity</span>
        <div className="flex gap-1">
          {filters.map(f => (
            <button key={f.id} onClick={() => setFilter(f.id)}
              className={`text-[10px] px-2 py-0.5 rounded-full ${filter === f.id ? "bg-[var(--color-accent)] text-white" : "bg-white/5 text-white/40 hover:text-white/60"}`}>{f.label}</button>
          ))}
        </div>
        <button onClick={load} className="ml-auto text-white/30 hover:text-white/60" title="Refresh"><RefreshCw size={14} className={loading ? "animate-spin" : ""} /></button>
      </div>

      <div className="flex-1 overflow-y-auto space-y-0.5">
        {items.map((item, i) => {
          const Icon = ICONS[item.type] || FileText;
          const color = item.type === "comment" ? "text-blue-400" : item.type === "sprint" ? "text-yellow-400" : item.type === "burn" ? "text-green-400" : "text-white/40";
          return (
            <div key={`${item.type}-${item.created_at}-${i}`} className="flex items-start gap-3 py-2 px-2 rounded hover:bg-white/5 group">
              <div className={`mt-0.5 shrink-0 ${color}`}><Icon size={14} /></div>
              <div className="flex-1 min-w-0">
                <div className="text-xs text-white/70">
                  <span className="font-medium text-[var(--color-accent)]">{item.user}</span>{" "}
                  <span>{describe(item)}</span>
                </div>
                {item.type === "comment" && item.content && (
                  <div className="text-[11px] text-white/30 mt-0.5 truncate">{item.content}</div>
                )}
                {item.detail && item.type === "audit" && (
                  <div className="text-[11px] text-white/30 mt-0.5">{item.detail}</div>
                )}
              </div>
              <span className="text-[10px] text-white/20 shrink-0">{timeAgo(item.created_at)}</span>
            </div>
          );
        })}
        {items.length === 0 && !loading && (
          <div className="text-center text-white/20 text-sm py-16">No activity yet</div>
        )}
      </div>
    </div>
  );
}
