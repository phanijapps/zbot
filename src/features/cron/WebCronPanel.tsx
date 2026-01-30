// ============================================================================
// WEB CRON PANEL
// Scheduled task management for web dashboard
// ============================================================================

import { useState } from "react";

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
    <div className="p-6 h-full overflow-auto">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold">Scheduled Tasks</h1>
          <p className="text-gray-500 text-sm mt-1">Automate agent invocations on a schedule</p>
        </div>
        <button
          onClick={() => setIsCreating(true)}
          className="bg-violet-600 hover:bg-violet-700 text-white px-4 py-2 rounded-lg transition-colors"
        >
          New Schedule
        </button>
      </div>

      {jobs.length === 0 ? (
        <div className="bg-[#141414] border border-gray-800 rounded-lg p-8 text-center">
          <div className="text-4xl mb-4">⏰</div>
          <h2 className="text-lg font-semibold mb-2">No Scheduled Tasks</h2>
          <p className="text-gray-500 mb-4">
            Create scheduled tasks to automatically invoke agents at specific times.
          </p>
          <button
            onClick={() => setIsCreating(true)}
            className="bg-violet-600 hover:bg-violet-700 text-white px-4 py-2 rounded-lg transition-colors"
          >
            Create First Schedule
          </button>
        </div>
      ) : (
        <div className="space-y-4">
          {jobs.map((job) => (
            <div
              key={job.id}
              className="bg-[#141414] border border-gray-800 rounded-lg p-4"
            >
              <div className="flex items-center justify-between">
                <div>
                  <h3 className="font-semibold">{job.name}</h3>
                  <p className="text-sm text-gray-500">{job.schedule}</p>
                </div>
                <div className="flex items-center gap-2">
                  <span
                    className={`px-2 py-1 text-xs rounded ${
                      job.enabled
                        ? "bg-green-900/30 text-green-400"
                        : "bg-gray-800 text-gray-500"
                    }`}
                  >
                    {job.enabled ? "Active" : "Paused"}
                  </span>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Coming Soon Notice */}
      <div className="mt-8 p-4 bg-violet-900/20 border border-violet-800 rounded-lg">
        <h3 className="font-semibold text-violet-400 mb-1">Coming Soon</h3>
        <p className="text-sm text-gray-400">
          Cron scheduling is currently being developed. You'll be able to:
        </p>
        <ul className="text-sm text-gray-400 mt-2 list-disc list-inside">
          <li>Schedule agents to run at specific times</li>
          <li>Use cron expressions for complex schedules</li>
          <li>View execution history and logs</li>
          <li>Set up alerts for failed executions</li>
        </ul>
      </div>
    </div>
  );
}
