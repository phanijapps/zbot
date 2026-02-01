// ============================================================================
// WEB CRON PANEL
// Scheduled task management for web dashboard
// ============================================================================

import { useState } from "react";
import { Calendar, Plus, Clock, Play, Pause, Bell, History, AlertTriangle } from "lucide-react";

// ============================================================================
// Types
// ============================================================================

interface CronJob {
  id: string;
  name: string;
  schedule: string;
  agentId: string;
  message: string;
  enabled: boolean;
  lastRun?: string;
  nextRun?: string;
}

// ============================================================================
// Component
// ============================================================================

export function WebCronPanel() {
  const [jobs] = useState<CronJob[]>([]);
  const [, setIsCreating] = useState(false);

  return (
    <div className="h-full overflow-auto bg-[var(--background)]">
      <div className="p-8 max-w-3xl mx-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-[var(--foreground)]">Scheduled Tasks</h1>
            <p className="text-[var(--muted-foreground)] text-sm mt-1">
              Automate agent invocations on a schedule
            </p>
          </div>
          <button
            onClick={() => setIsCreating(true)}
            className="inline-flex items-center gap-2 bg-[var(--primary)] hover:bg-[var(--primary)]/90 text-white px-4 py-2 rounded-lg transition-colors text-sm font-medium"
          >
            <Plus className="w-4 h-4" />
            New Schedule
          </button>
        </div>

        {jobs.length === 0 ? (
          <div className="bg-[var(--card)] rounded-xl p-10 text-center card-shadow">
            <div className="w-12 h-12 rounded-xl bg-[var(--primary)]/10 flex items-center justify-center mx-auto mb-4">
              <Calendar className="w-6 h-6 text-[var(--primary)]" />
            </div>
            <h2 className="text-base font-semibold text-[var(--foreground)] mb-1">No Scheduled Tasks</h2>
            <p className="text-[var(--muted-foreground)] text-sm mb-5">
              Create scheduled tasks to automatically invoke agents at specific times.
            </p>
            <button
              onClick={() => setIsCreating(true)}
              className="inline-flex items-center gap-2 bg-[var(--primary)] hover:bg-[var(--primary)]/90 text-white px-4 py-2 rounded-lg transition-colors text-sm font-medium"
            >
              <Plus className="w-4 h-4" />
              Create First Schedule
            </button>
          </div>
        ) : (
          <div className="space-y-3">
            {jobs.map((job) => (
              <div
                key={job.id}
                className="bg-[var(--card)] rounded-xl p-4 card-shadow hover:shadow-md transition-shadow"
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <div className={`w-9 h-9 rounded-lg flex items-center justify-center ${
                      job.enabled ? "bg-[var(--success)]/10" : "bg-[var(--muted)]"
                    }`}>
                      {job.enabled ? (
                        <Play className="w-4 h-4 text-[var(--success)]" />
                      ) : (
                        <Pause className="w-4 h-4 text-[var(--muted-foreground)]" />
                      )}
                    </div>
                    <div>
                      <h3 className="font-medium text-sm text-[var(--foreground)]">{job.name}</h3>
                      <div className="flex items-center gap-1.5 text-xs text-[var(--muted-foreground)]">
                        <Clock className="w-3 h-3" />
                        {job.schedule}
                      </div>
                    </div>
                  </div>
                  <span
                    className={`inline-flex items-center gap-1 px-2 py-0.5 text-xs font-medium rounded ${
                      job.enabled
                        ? "bg-[var(--success)]/10 text-[var(--success)]"
                        : "bg-[var(--muted)] text-[var(--muted-foreground)]"
                    }`}
                  >
                    {job.enabled ? "Active" : "Paused"}
                  </span>
                </div>
              </div>
            ))}
          </div>
        )}

        {/* Coming Soon Notice */}
        <div className="mt-6 bg-[var(--card)] rounded-xl p-5 card-shadow">
          <div className="flex items-start gap-3">
            <div className="w-9 h-9 rounded-lg bg-[var(--warning)]/10 flex items-center justify-center flex-shrink-0">
              <AlertTriangle className="w-4.5 h-4.5 text-[var(--warning)]" />
            </div>
            <div>
              <h3 className="font-medium text-sm text-[var(--foreground)] mb-2">Coming Soon</h3>
              <p className="text-sm text-[var(--muted-foreground)] mb-3">
                Cron scheduling is currently being developed. You'll be able to:
              </p>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                <div className="flex items-center gap-2 text-xs text-[var(--foreground)]">
                  <Clock className="w-3.5 h-3.5 text-[var(--primary)]" />
                  Schedule agents to run at specific times
                </div>
                <div className="flex items-center gap-2 text-xs text-[var(--foreground)]">
                  <Calendar className="w-3.5 h-3.5 text-[var(--primary)]" />
                  Use cron expressions for complex schedules
                </div>
                <div className="flex items-center gap-2 text-xs text-[var(--foreground)]">
                  <History className="w-3.5 h-3.5 text-[var(--primary)]" />
                  View execution history and logs
                </div>
                <div className="flex items-center gap-2 text-xs text-[var(--foreground)]">
                  <Bell className="w-3.5 h-3.5 text-[var(--primary)]" />
                  Set up alerts for failed executions
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
