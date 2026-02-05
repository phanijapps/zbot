// ============================================================================
// WEB CRON PANEL
// Scheduled task management for web dashboard
// ============================================================================

import { useState, useEffect, useCallback } from "react";
import {
  Calendar,
  Plus,
  Clock,
  Play,
  Pause,
  Trash2,
  Edit2,
  X,
  ChevronRight,
  RefreshCw,
  CheckCircle,
  XCircle,
  AlertTriangle,
  Zap,
  Info,
} from "lucide-react";
import { getTransport, type Transport } from "@/services/transport";
import type {
  CronJobResponse,
  CreateCronJobRequest,
  UpdateCronJobRequest,
} from "@/services/transport/types";

// ============================================================================
// Types
// ============================================================================

interface CronDialogState {
  open: boolean;
  mode: "create" | "edit";
  editingId?: string;
}

// Common cron presets for easy selection
const CRON_PRESETS = [
  { label: "Every minute", value: "* * * * *" },
  { label: "Every 5 minutes", value: "*/5 * * * *" },
  { label: "Every 15 minutes", value: "*/15 * * * *" },
  { label: "Every hour", value: "0 * * * *" },
  { label: "Every 6 hours", value: "0 */6 * * *" },
  { label: "Daily at midnight", value: "0 0 * * *" },
  { label: "Daily at 9 AM", value: "0 9 * * *" },
  { label: "Weekly on Monday", value: "0 0 * * 1" },
  { label: "Monthly on the 1st", value: "0 0 1 * *" },
];

// ============================================================================
// Component
// ============================================================================

export function WebCronPanel() {
  // State
  const [jobs, setJobs] = useState<CronJobResponse[]>([]);
  const [selectedJobId, setSelectedJobId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [dialogState, setDialogState] = useState<CronDialogState>({
    open: false,
    mode: "create",
  });
  const [triggeringJob, setTriggeringJob] = useState<string | null>(null);
  const [transport, setTransport] = useState<Transport | null>(null);

  // Initialize transport
  useEffect(() => {
    getTransport().then(setTransport);
  }, []);

  // Form state (agent_id always defaults to "root" - scheduled tasks go to root agent)
  const [formData, setFormData] = useState({
    id: "",
    name: "",
    schedule: "0 * * * *",
    message: "",
    enabled: true,
    timezone: "",
  });

  // Selected job
  const selectedJob = jobs.find((j) => j.id === selectedJobId);

  // Load jobs
  const loadData = useCallback(async () => {
    if (!transport) return;

    setLoading(true);
    setError(null);

    try {
      const jobsResult = await transport.listCronJobs();

      if (jobsResult.success && jobsResult.data) {
        setJobs(jobsResult.data);
        // Select first job if none selected
        if (!selectedJobId && jobsResult.data.length > 0) {
          setSelectedJobId(jobsResult.data[0].id);
        }
      } else {
        setError(jobsResult.error || "Failed to load cron jobs");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load data");
    } finally {
      setLoading(false);
    }
  }, [transport, selectedJobId]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  // Open create dialog
  const openCreateDialog = () => {
    setFormData({
      id: "",
      name: "",
      schedule: "0 * * * *",
      message: "",
      enabled: true,
      timezone: "",
    });
    setDialogState({ open: true, mode: "create" });
  };

  // Open edit dialog
  const openEditDialog = (job: CronJobResponse) => {
    setFormData({
      id: job.id,
      name: job.name,
      schedule: job.schedule,
      message: job.message,
      enabled: job.enabled,
      timezone: job.timezone || "",
    });
    setDialogState({ open: true, mode: "edit", editingId: job.id });
  };

  // Close dialog
  const closeDialog = () => {
    setDialogState({ open: false, mode: "create" });
  };

  // Generate ID from name
  const generateId = (name: string) => {
    return name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "");
  };

  // Handle name change (auto-generate ID in create mode)
  const handleNameChange = (name: string) => {
    setFormData((prev) => ({
      ...prev,
      name,
      id: dialogState.mode === "create" ? generateId(name) : prev.id,
    }));
  };

  // Submit form
  const handleSubmit = async () => {
    if (!transport) return;

    try {
      if (dialogState.mode === "create") {
        // Always use "root" as agent_id - scheduled tasks go to root agent
        const request: CreateCronJobRequest = {
          id: formData.id,
          name: formData.name,
          schedule: formData.schedule,
          agent_id: "root",
          message: formData.message,
          enabled: formData.enabled,
          timezone: formData.timezone || undefined,
        };

        const result = await transport.createCronJob(request);
        if (result.success && result.data) {
          setJobs((prev) => [...prev, result.data!]);
          setSelectedJobId(result.data.id);
          closeDialog();
        } else {
          setError(result.error || "Failed to create cron job");
        }
      } else {
        const request: UpdateCronJobRequest = {
          name: formData.name,
          schedule: formData.schedule,
          message: formData.message,
          timezone: formData.timezone || undefined,
        };

        const result = await transport.updateCronJob(
          dialogState.editingId!,
          request
        );
        if (result.success && result.data) {
          setJobs((prev) =>
            prev.map((j) => (j.id === dialogState.editingId ? result.data! : j))
          );
          closeDialog();
        } else {
          setError(result.error || "Failed to update cron job");
        }
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Operation failed");
    }
  };

  // Delete job
  const handleDelete = async (id: string) => {
    if (!transport) return;

    try {
      const result = await transport.deleteCronJob(id);
      if (result.success) {
        setJobs((prev) => prev.filter((j) => j.id !== id));
        if (selectedJobId === id) {
          setSelectedJobId(jobs.find((j) => j.id !== id)?.id || null);
        }
      } else {
        setError(result.error || "Failed to delete cron job");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Delete failed");
    }
  };

  // Toggle enabled
  const handleToggleEnabled = async (job: CronJobResponse) => {
    if (!transport) return;

    try {
      const result = job.enabled
        ? await transport.disableCronJob(job.id)
        : await transport.enableCronJob(job.id);

      if (result.success && result.data) {
        setJobs((prev) =>
          prev.map((j) => (j.id === job.id ? result.data! : j))
        );
      } else {
        setError(result.error || "Failed to toggle cron job");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Toggle failed");
    }
  };

  // Trigger job manually
  const handleTrigger = async (id: string) => {
    if (!transport) return;

    setTriggeringJob(id);
    try {
      const result = await transport.triggerCronJob(id);
      if (result.success) {
        // Reload to get updated last_run
        await loadData();
      } else {
        setError(result.error || "Failed to trigger cron job");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Trigger failed");
    } finally {
      setTriggeringJob(null);
    }
  };

  // Parse cron expression for display
  const describeCron = (schedule: string): string => {
    const preset = CRON_PRESETS.find((p) => p.value === schedule);
    if (preset) return preset.label;
    return schedule;
  };

  // Format timestamp
  const formatTime = (timestamp?: string) => {
    if (!timestamp) return "Never";
    const date = new Date(timestamp);
    return date.toLocaleString();
  };

  // Loading state
  if (loading && jobs.length === 0) {
    return (
      <div className="h-full flex items-center justify-center bg-[var(--background)]">
        <div className="flex flex-col items-center gap-3">
          <RefreshCw className="w-6 h-6 text-[var(--muted-foreground)] animate-spin" />
          <span className="text-sm text-[var(--muted-foreground)]">
            Loading scheduled tasks...
          </span>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex bg-[var(--background)]">
      {/* Left Panel - Job List */}
      <div className="w-80 border-r border-[var(--border)] flex flex-col">
        {/* Header */}
        <div className="p-4 border-b border-[var(--border)]">
          <div className="flex items-center justify-between mb-3">
            <h2 className="font-semibold text-[var(--foreground)]">
              Scheduled Tasks
            </h2>
            <button
              onClick={openCreateDialog}
              className="p-1.5 rounded-lg bg-[var(--primary)] text-white hover:bg-[var(--primary)]/90 transition-colors"
              title="New Schedule"
            >
              <Plus className="w-4 h-4" />
            </button>
          </div>
          <p className="text-xs text-[var(--muted-foreground)]">
            Automate agent invocations on a schedule
          </p>
        </div>

        {/* Error Banner */}
        {error && (
          <div className="mx-4 mt-4 p-3 bg-[var(--error)]/10 border border-[var(--error)]/20 rounded-lg">
            <div className="flex items-start gap-2">
              <AlertTriangle className="w-4 h-4 text-[var(--error)] flex-shrink-0 mt-0.5" />
              <div className="flex-1 min-w-0">
                <p className="text-sm text-[var(--error)]">{error}</p>
                <button
                  onClick={() => setError(null)}
                  className="text-xs text-[var(--error)] underline mt-1"
                >
                  Dismiss
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Job List */}
        <div className="flex-1 overflow-auto p-4">
          {jobs.length === 0 ? (
            <div className="text-center py-8">
              <div className="w-12 h-12 rounded-xl bg-[var(--primary)]/10 flex items-center justify-center mx-auto mb-3">
                <Calendar className="w-6 h-6 text-[var(--primary)]" />
              </div>
              <h3 className="text-sm font-medium text-[var(--foreground)] mb-1">
                No Scheduled Tasks
              </h3>
              <p className="text-xs text-[var(--muted-foreground)] mb-4">
                Create your first scheduled task
              </p>
              <button
                onClick={openCreateDialog}
                className="inline-flex items-center gap-2 bg-[var(--primary)] hover:bg-[var(--primary)]/90 text-white px-3 py-2 rounded-lg transition-colors text-sm"
              >
                <Plus className="w-4 h-4" />
                New Schedule
              </button>
            </div>
          ) : (
            <div className="space-y-2">
              {jobs.map((job) => (
                <button
                  key={job.id}
                  onClick={() => setSelectedJobId(job.id)}
                  className={`w-full text-left p-3 rounded-lg transition-all ${
                    selectedJobId === job.id
                      ? "bg-[var(--primary)]/10 border border-[var(--primary)]/30"
                      : "bg-[var(--card)] hover:bg-[var(--muted)] border border-transparent"
                  }`}
                >
                  <div className="flex items-center gap-3">
                    <div
                      className={`w-8 h-8 rounded-lg flex items-center justify-center ${
                        job.enabled
                          ? "bg-[var(--success)]/10"
                          : "bg-[var(--muted)]"
                      }`}
                    >
                      {job.enabled ? (
                        <Play className="w-4 h-4 text-[var(--success)]" />
                      ) : (
                        <Pause className="w-4 h-4 text-[var(--muted-foreground)]" />
                      )}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="font-medium text-sm text-[var(--foreground)] truncate">
                          {job.name}
                        </span>
                        {selectedJobId === job.id && (
                          <ChevronRight className="w-3 h-3 text-[var(--primary)]" />
                        )}
                      </div>
                      <div className="flex items-center gap-1 text-xs text-[var(--muted-foreground)]">
                        <Clock className="w-3 h-3" />
                        {describeCron(job.schedule)}
                      </div>
                    </div>
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Right Panel - Job Details */}
      <div className="flex-1 overflow-auto">
        {selectedJob ? (
          <div className="p-6">
            {/* Job Header */}
            <div className="flex items-start justify-between mb-6">
              <div className="flex items-center gap-4">
                <div
                  className={`w-12 h-12 rounded-xl flex items-center justify-center ${
                    selectedJob.enabled
                      ? "bg-[var(--success)]/10"
                      : "bg-[var(--muted)]"
                  }`}
                >
                  {selectedJob.enabled ? (
                    <Play className="w-6 h-6 text-[var(--success)]" />
                  ) : (
                    <Pause className="w-6 h-6 text-[var(--muted-foreground)]" />
                  )}
                </div>
                <div>
                  <h1 className="text-xl font-semibold text-[var(--foreground)]">
                    {selectedJob.name}
                  </h1>
                  <p className="text-sm text-[var(--muted-foreground)]">
                    {selectedJob.id}
                  </p>
                </div>
              </div>

              <div className="flex items-center gap-2">
                <button
                  onClick={() => handleTrigger(selectedJob.id)}
                  disabled={triggeringJob === selectedJob.id}
                  className="inline-flex items-center gap-2 px-3 py-2 bg-[var(--primary)] text-white rounded-lg hover:bg-[var(--primary)]/90 transition-colors text-sm disabled:opacity-50"
                >
                  {triggeringJob === selectedJob.id ? (
                    <RefreshCw className="w-4 h-4 animate-spin" />
                  ) : (
                    <Zap className="w-4 h-4" />
                  )}
                  Trigger Now
                </button>
                <button
                  onClick={() => openEditDialog(selectedJob)}
                  className="p-2 rounded-lg bg-[var(--card)] hover:bg-[var(--muted)] transition-colors"
                  title="Edit"
                >
                  <Edit2 className="w-4 h-4 text-[var(--foreground)]" />
                </button>
                <button
                  onClick={() => handleDelete(selectedJob.id)}
                  className="p-2 rounded-lg bg-[var(--card)] hover:bg-[var(--error)]/10 transition-colors"
                  title="Delete"
                >
                  <Trash2 className="w-4 h-4 text-[var(--error)]" />
                </button>
              </div>
            </div>

            {/* Status Toggle */}
            <div className="bg-[var(--card)] rounded-xl p-4 mb-4 card-shadow">
              <div className="flex items-center justify-between">
                <div>
                  <h3 className="font-medium text-sm text-[var(--foreground)]">
                    Status
                  </h3>
                  <p className="text-xs text-[var(--muted-foreground)]">
                    {selectedJob.enabled
                      ? "This job is active and will run on schedule"
                      : "This job is paused and will not run"}
                  </p>
                </div>
                <button
                  onClick={() => handleToggleEnabled(selectedJob)}
                  className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                    selectedJob.enabled
                      ? "bg-[var(--success)]"
                      : "bg-[var(--muted)]"
                  }`}
                >
                  <span
                    className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                      selectedJob.enabled ? "translate-x-6" : "translate-x-1"
                    }`}
                  />
                </button>
              </div>
            </div>

            {/* Schedule Info */}
            <div className="bg-[var(--card)] rounded-xl p-4 mb-4 card-shadow">
              <h3 className="font-medium text-sm text-[var(--foreground)] mb-3">
                Schedule
              </h3>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="text-xs text-[var(--muted-foreground)]">
                    Cron Expression
                  </label>
                  <p className="font-mono text-sm text-[var(--foreground)] mt-1">
                    {selectedJob.schedule}
                  </p>
                </div>
                <div>
                  <label className="text-xs text-[var(--muted-foreground)]">
                    Human Readable
                  </label>
                  <p className="text-sm text-[var(--foreground)] mt-1">
                    {describeCron(selectedJob.schedule)}
                  </p>
                </div>
                {selectedJob.timezone && (
                  <div>
                    <label className="text-xs text-[var(--muted-foreground)]">
                      Timezone
                    </label>
                    <p className="text-sm text-[var(--foreground)] mt-1">
                      {selectedJob.timezone}
                    </p>
                  </div>
                )}
              </div>
            </div>

            {/* Message */}
            <div className="bg-[var(--card)] rounded-xl p-4 mb-4 card-shadow">
              <h3 className="font-medium text-sm text-[var(--foreground)] mb-3">
                Message
              </h3>
              <p className="text-sm text-[var(--foreground)] bg-[var(--muted)] p-3 rounded-lg">
                {selectedJob.message}
              </p>
              <p className="text-xs text-[var(--muted-foreground)] mt-2">
                This message will be sent to the root agent when triggered
              </p>
            </div>

            {/* Execution History */}
            <div className="bg-[var(--card)] rounded-xl p-4 card-shadow">
              <h3 className="font-medium text-sm text-[var(--foreground)] mb-3">
                Execution Info
              </h3>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="text-xs text-[var(--muted-foreground)]">
                    Last Run
                  </label>
                  <div className="flex items-center gap-2 mt-1">
                    {selectedJob.last_run ? (
                      <>
                        <CheckCircle className="w-4 h-4 text-[var(--success)]" />
                        <span className="text-sm text-[var(--foreground)]">
                          {formatTime(selectedJob.last_run)}
                        </span>
                      </>
                    ) : (
                      <>
                        <Info className="w-4 h-4 text-[var(--muted-foreground)]" />
                        <span className="text-sm text-[var(--muted-foreground)]">
                          Never
                        </span>
                      </>
                    )}
                  </div>
                </div>
                <div>
                  <label className="text-xs text-[var(--muted-foreground)]">
                    Next Run
                  </label>
                  <div className="flex items-center gap-2 mt-1">
                    {selectedJob.enabled && selectedJob.next_run ? (
                      <>
                        <Clock className="w-4 h-4 text-[var(--primary)]" />
                        <span className="text-sm text-[var(--foreground)]">
                          {formatTime(selectedJob.next_run)}
                        </span>
                      </>
                    ) : (
                      <>
                        <XCircle className="w-4 h-4 text-[var(--muted-foreground)]" />
                        <span className="text-sm text-[var(--muted-foreground)]">
                          {selectedJob.enabled ? "Calculating..." : "Paused"}
                        </span>
                      </>
                    )}
                  </div>
                </div>
                {selectedJob.created_at && (
                  <div>
                    <label className="text-xs text-[var(--muted-foreground)]">
                      Created
                    </label>
                    <p className="text-sm text-[var(--foreground)] mt-1">
                      {formatTime(selectedJob.created_at)}
                    </p>
                  </div>
                )}
              </div>
            </div>
          </div>
        ) : (
          <div className="h-full flex items-center justify-center">
            <div className="text-center">
              <div className="w-16 h-16 rounded-2xl bg-[var(--muted)] flex items-center justify-center mx-auto mb-4">
                <Calendar className="w-8 h-8 text-[var(--muted-foreground)]" />
              </div>
              <h3 className="text-lg font-medium text-[var(--foreground)] mb-2">
                No Task Selected
              </h3>
              <p className="text-sm text-[var(--muted-foreground)]">
                Select a scheduled task to view details
              </p>
            </div>
          </div>
        )}
      </div>

      {/* Create/Edit Dialog */}
      {dialogState.open && (
        <div className="fixed inset-0 z-50 flex items-center justify-center">
          {/* Backdrop */}
          <div
            className="absolute inset-0 bg-black/50"
            onClick={closeDialog}
          />

          {/* Dialog */}
          <div className="relative bg-[var(--card)] rounded-2xl shadow-xl w-full max-w-lg mx-4 max-h-[90vh] overflow-auto">
            {/* Header */}
            <div className="flex items-center justify-between p-4 border-b border-[var(--border)]">
              <h2 className="text-lg font-semibold text-[var(--foreground)]">
                {dialogState.mode === "create"
                  ? "New Scheduled Task"
                  : "Edit Scheduled Task"}
              </h2>
              <button
                onClick={closeDialog}
                className="p-1.5 rounded-lg hover:bg-[var(--muted)] transition-colors"
              >
                <X className="w-5 h-5 text-[var(--muted-foreground)]" />
              </button>
            </div>

            {/* Body */}
            <div className="p-4 space-y-4">
              {/* Name */}
              <div>
                <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">
                  Name
                </label>
                <input
                  type="text"
                  value={formData.name}
                  onChange={(e) => handleNameChange(e.target.value)}
                  placeholder="My Scheduled Task"
                  className="w-full px-3 py-2 bg-[var(--background)] border border-[var(--border)] rounded-lg text-[var(--foreground)] placeholder:text-[var(--muted-foreground)] focus:outline-none focus:ring-2 focus:ring-[var(--primary)]/50"
                />
              </div>

              {/* ID (readonly in edit mode) */}
              <div>
                <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">
                  ID
                </label>
                <input
                  type="text"
                  value={formData.id}
                  onChange={(e) =>
                    dialogState.mode === "create" &&
                    setFormData((prev) => ({ ...prev, id: e.target.value }))
                  }
                  placeholder="my-scheduled-task"
                  disabled={dialogState.mode === "edit"}
                  className="w-full px-3 py-2 bg-[var(--background)] border border-[var(--border)] rounded-lg text-[var(--foreground)] placeholder:text-[var(--muted-foreground)] focus:outline-none focus:ring-2 focus:ring-[var(--primary)]/50 disabled:opacity-50"
                />
              </div>

              {/* Schedule */}
              <div>
                <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">
                  Schedule (Cron Expression)
                </label>
                <div className="flex gap-2 mb-2">
                  <select
                    value={
                      CRON_PRESETS.find((p) => p.value === formData.schedule)
                        ? formData.schedule
                        : "custom"
                    }
                    onChange={(e) => {
                      if (e.target.value !== "custom") {
                        setFormData((prev) => ({
                          ...prev,
                          schedule: e.target.value,
                        }));
                      }
                    }}
                    className="flex-1 px-3 py-2 bg-[var(--background)] border border-[var(--border)] rounded-lg text-[var(--foreground)] focus:outline-none focus:ring-2 focus:ring-[var(--primary)]/50"
                  >
                    {CRON_PRESETS.map((preset) => (
                      <option key={preset.value} value={preset.value}>
                        {preset.label}
                      </option>
                    ))}
                    <option value="custom">Custom...</option>
                  </select>
                </div>
                <input
                  type="text"
                  value={formData.schedule}
                  onChange={(e) =>
                    setFormData((prev) => ({ ...prev, schedule: e.target.value }))
                  }
                  placeholder="* * * * *"
                  className="w-full px-3 py-2 bg-[var(--background)] border border-[var(--border)] rounded-lg text-[var(--foreground)] font-mono placeholder:text-[var(--muted-foreground)] focus:outline-none focus:ring-2 focus:ring-[var(--primary)]/50"
                />
                <p className="text-xs text-[var(--muted-foreground)] mt-1">
                  Format: minute hour day-of-month month day-of-week
                </p>
              </div>

              {/* Message */}
              <div>
                <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">
                  Message
                </label>
                <textarea
                  value={formData.message}
                  onChange={(e) =>
                    setFormData((prev) => ({ ...prev, message: e.target.value }))
                  }
                  placeholder="The message to send to the agent..."
                  rows={3}
                  className="w-full px-3 py-2 bg-[var(--background)] border border-[var(--border)] rounded-lg text-[var(--foreground)] placeholder:text-[var(--muted-foreground)] focus:outline-none focus:ring-2 focus:ring-[var(--primary)]/50 resize-none"
                />
              </div>

              {/* Timezone (optional) */}
              <div>
                <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">
                  Timezone (optional)
                </label>
                <input
                  type="text"
                  value={formData.timezone}
                  onChange={(e) =>
                    setFormData((prev) => ({ ...prev, timezone: e.target.value }))
                  }
                  placeholder="America/New_York"
                  className="w-full px-3 py-2 bg-[var(--background)] border border-[var(--border)] rounded-lg text-[var(--foreground)] placeholder:text-[var(--muted-foreground)] focus:outline-none focus:ring-2 focus:ring-[var(--primary)]/50"
                />
                <p className="text-xs text-[var(--muted-foreground)] mt-1">
                  Leave empty to use system timezone
                </p>
              </div>

              {/* Enabled (create only) */}
              {dialogState.mode === "create" && (
                <div className="flex items-center justify-between p-3 bg-[var(--muted)] rounded-lg">
                  <div>
                    <label className="text-sm font-medium text-[var(--foreground)]">
                      Enable on Create
                    </label>
                    <p className="text-xs text-[var(--muted-foreground)]">
                      Start running immediately after creation
                    </p>
                  </div>
                  <button
                    type="button"
                    onClick={() =>
                      setFormData((prev) => ({ ...prev, enabled: !prev.enabled }))
                    }
                    className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                      formData.enabled
                        ? "bg-[var(--success)]"
                        : "bg-[var(--border)]"
                    }`}
                  >
                    <span
                      className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                        formData.enabled ? "translate-x-6" : "translate-x-1"
                      }`}
                    />
                  </button>
                </div>
              )}
            </div>

            {/* Footer */}
            <div className="flex items-center justify-end gap-3 p-4 border-t border-[var(--border)]">
              <button
                onClick={closeDialog}
                className="px-4 py-2 rounded-lg text-sm font-medium text-[var(--foreground)] hover:bg-[var(--muted)] transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleSubmit}
                disabled={
                  !formData.name ||
                  !formData.id ||
                  !formData.schedule ||
                  !formData.message
                }
                className="px-4 py-2 rounded-lg text-sm font-medium bg-[var(--primary)] text-white hover:bg-[var(--primary)]/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {dialogState.mode === "create" ? "Create" : "Save"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
