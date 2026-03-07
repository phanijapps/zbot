// ============================================================================
// WEB SKILLS PANEL
// Skill management for web dashboard (uses transport layer)
// ============================================================================

import { useState, useEffect } from "react";
import { Zap, Plus, Trash2, FolderOpen, FileText, Loader2, X } from "lucide-react";
import { getTransport, type SkillResponse, type CreateSkillRequest } from "@/services/transport";

// ============================================================================
// Component
// ============================================================================

export function WebSkillsPanel() {
  const [skills, setSkills] = useState<SkillResponse[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [selectedSkill, setSelectedSkill] = useState<SkillResponse | null>(null);
  const [newSkill, setNewSkill] = useState<Partial<CreateSkillRequest>>({
    name: "",
    displayName: "",
    description: "",
    category: "general",
    instructions: "You are a helpful skill.",
  });

  useEffect(() => {
    loadSkills();
  }, []);

  const loadSkills = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const transport = await getTransport();
      const result = await transport.listSkills();
      if (result.success && result.data) {
        setSkills(result.data);
      } else {
        setError(result.error || "Failed to load skills");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setIsLoading(false);
    }
  };

  const handleCreateSkill = async () => {
    if (!newSkill.name) return;

    try {
      const transport = await getTransport();
      const result = await transport.createSkill({
        name: newSkill.name,
        displayName: newSkill.displayName || newSkill.name,
        description: newSkill.description,
        category: newSkill.category || "general",
        instructions: newSkill.instructions,
      });

      if (result.success) {
        setIsCreating(false);
        setNewSkill({
          name: "",
          displayName: "",
          description: "",
          category: "general",
          instructions: "You are a helpful skill.",
        });
        loadSkills();
      } else {
        setError(result.error || "Failed to create skill");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    }
  };

  const handleDeleteSkill = async (id: string) => {
    if (!confirm("Are you sure you want to delete this skill?")) return;

    try {
      const transport = await getTransport();
      const result = await transport.deleteSkill(id);
      if (result.success) {
        setSelectedSkill(null);
        loadSkills();
      } else {
        setError(result.error || "Failed to delete skill");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    }
  };

  // Group skills by category
  const skillsByCategory = skills.reduce(
    (acc, skill) => {
      const category = skill.category || "general";
      if (!acc[category]) acc[category] = [];
      acc[category].push(skill);
      return acc;
    },
    {} as Record<string, SkillResponse[]>
  );

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full bg-[var(--background)]">
        <Loader2 className="w-6 h-6 text-[var(--primary)] animate-spin" />
      </div>
    );
  }

  return (
    <div className="flex h-full bg-[var(--background)]">
      {/* Skills List */}
      <div className="w-72 bg-[var(--card)] border-r border-[var(--border)] flex flex-col">
        <div className="p-4 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Zap className="w-4 h-4 text-[var(--warning)]" />
            <h1 className="text-sm font-semibold text-[var(--foreground)]">Skills</h1>
          </div>
          <button
            onClick={() => setIsCreating(true)}
            className="inline-flex items-center gap-1 bg-[var(--primary)] hover:bg-[var(--primary)]/90 text-[var(--primary-foreground)] px-2.5 py-1.5 rounded-lg text-xs transition-colors font-medium"
          >
            <Plus className="w-3.5 h-3.5" />
            New
          </button>
        </div>

        {error && (
          <div className="px-3 py-2 bg-[var(--destructive)]/10 text-[var(--destructive)] text-xs flex items-center justify-between">
            <span className="truncate">{error}</span>
            <button onClick={() => setError(null)} className="hover:opacity-70 ml-2">
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        )}

        <div className="flex-1 overflow-auto">
          {skills.length === 0 ? (
            <div className="p-6 text-center">
              <div className="w-10 h-10 rounded-lg bg-[var(--warning)]/10 flex items-center justify-center mx-auto mb-3">
                <Zap className="w-5 h-5 text-[var(--warning)]" />
              </div>
              <p className="text-sm font-medium text-[var(--foreground)]">No skills yet</p>
              <p className="text-xs text-[var(--muted-foreground)] mt-1">Create your first skill</p>
            </div>
          ) : (
            Object.entries(skillsByCategory).map(([category, categorySkills]) => (
              <div key={category}>
                <div className="px-3 py-2 text-xs text-[var(--muted-foreground)] uppercase tracking-wider bg-[var(--muted)] flex items-center gap-1.5">
                  <FolderOpen className="w-3 h-3" />
                  {category}
                </div>
                {categorySkills.map((skill) => (
                  <button
                    key={skill.id}
                    onClick={() => setSelectedSkill(skill)}
                    className={`w-full text-left px-3 py-2.5 hover:bg-[var(--muted)] transition-colors ${
                      selectedSkill?.id === skill.id ? "bg-[var(--accent)] border-l-2 border-l-[var(--primary)]" : ""
                    }`}
                  >
                    <div className="text-sm font-medium text-[var(--foreground)]">{skill.displayName}</div>
                    <div className="text-xs text-[var(--muted-foreground)] truncate">{skill.description}</div>
                  </button>
                ))}
              </div>
            ))
          )}
        </div>
      </div>

      {/* Skill Detail / Create Form */}
      <div className="flex-1 overflow-auto">
        {isCreating ? (
          <div className="p-8 max-w-xl">
            <div className="flex items-center gap-3 mb-5">
              <div className="w-9 h-9 rounded-lg bg-[var(--warning)]/10 flex items-center justify-center">
                <Zap className="w-4.5 h-4.5 text-[var(--warning)]" />
              </div>
              <h2 className="text-lg font-semibold text-[var(--foreground)]">Create New Skill</h2>
            </div>

            <div className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Name (ID)</label>
                <input
                  type="text"
                  value={newSkill.name}
                  onChange={(e) =>
                    setNewSkill({
                      ...newSkill,
                      name: e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, "-"),
                    })
                  }
                  placeholder="my-skill"
                  className="w-full bg-[var(--background)] border border-[var(--border)] rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent text-[var(--foreground)] text-sm"
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Display Name</label>
                <input
                  type="text"
                  value={newSkill.displayName}
                  onChange={(e) => setNewSkill({ ...newSkill, displayName: e.target.value })}
                  placeholder="My Skill"
                  className="w-full bg-[var(--background)] border border-[var(--border)] rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent text-[var(--foreground)] text-sm"
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Description</label>
                <input
                  type="text"
                  value={newSkill.description}
                  onChange={(e) => setNewSkill({ ...newSkill, description: e.target.value })}
                  placeholder="What does this skill do?"
                  className="w-full bg-[var(--background)] border border-[var(--border)] rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent text-[var(--foreground)] text-sm"
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Category</label>
                <input
                  type="text"
                  value={newSkill.category}
                  onChange={(e) => setNewSkill({ ...newSkill, category: e.target.value })}
                  placeholder="general"
                  className="w-full bg-[var(--background)] border border-[var(--border)] rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent text-[var(--foreground)] text-sm"
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Instructions</label>
                <textarea
                  value={newSkill.instructions}
                  onChange={(e) => setNewSkill({ ...newSkill, instructions: e.target.value })}
                  placeholder="Instructions for the agent when using this skill..."
                  rows={8}
                  className="w-full bg-[var(--background)] border border-[var(--border)] rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent resize-none font-mono text-sm text-[var(--foreground)]"
                />
              </div>

              <div className="flex gap-2 pt-2">
                <button
                  onClick={() => setIsCreating(false)}
                  className="px-4 py-2 text-[var(--muted-foreground)] hover:text-[var(--foreground)] transition-colors text-sm font-medium"
                >
                  Cancel
                </button>
                <button
                  onClick={handleCreateSkill}
                  disabled={!newSkill.name}
                  className="bg-[var(--primary)] hover:bg-[var(--primary)]/90 disabled:opacity-50 text-[var(--primary-foreground)] px-4 py-2 rounded-lg transition-colors text-sm font-medium"
                >
                  Create Skill
                </button>
              </div>
            </div>
          </div>
        ) : selectedSkill ? (
          <div className="p-8 max-w-xl">
            <div className="flex items-start justify-between mb-5">
              <div className="flex items-center gap-3">
                <div className="w-9 h-9 rounded-lg bg-[var(--warning)]/10 flex items-center justify-center">
                  <Zap className="w-4.5 h-4.5 text-[var(--warning)]" />
                </div>
                <div>
                  <h2 className="text-lg font-semibold text-[var(--foreground)]">{selectedSkill.displayName}</h2>
                  <p className="text-xs text-[var(--muted-foreground)]">{selectedSkill.id}</p>
                </div>
              </div>
              <button
                onClick={() => handleDeleteSkill(selectedSkill.id)}
                className="text-[var(--muted-foreground)] hover:text-[var(--destructive)] transition-colors p-1.5 hover:bg-[var(--destructive)]/10 rounded-lg"
              >
                <Trash2 className="w-4 h-4" />
              </button>
            </div>

            <div className="space-y-4">
              <div className="bg-[var(--card)] rounded-xl p-4 card-shadow">
                <label className="block text-xs text-[var(--muted-foreground)] uppercase tracking-wider mb-1">Description</label>
                <p className="text-sm text-[var(--foreground)]">{selectedSkill.description || "No description"}</p>
              </div>

              <div className="bg-[var(--card)] rounded-xl p-4 card-shadow">
                <label className="block text-xs text-[var(--muted-foreground)] uppercase tracking-wider mb-1">Category</label>
                <span className="inline-flex items-center gap-1 px-2 py-0.5 bg-[var(--muted)] rounded text-xs text-[var(--muted-foreground)]">
                  <FolderOpen className="w-3 h-3" />
                  {selectedSkill.category}
                </span>
              </div>

              <div className="bg-[var(--card)] rounded-xl p-4 card-shadow">
                <label className="block text-xs text-[var(--muted-foreground)] uppercase tracking-wider mb-2 flex items-center gap-1">
                  <FileText className="w-3 h-3" />
                  Instructions
                </label>
                <pre className="bg-[var(--muted)] rounded-lg p-3 text-sm text-[var(--foreground)] whitespace-pre-wrap font-mono overflow-auto max-h-80">
                  {selectedSkill.instructions}
                </pre>
              </div>
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-center h-full">
            <div className="text-center">
              <div className="w-12 h-12 rounded-xl bg-[var(--muted)] flex items-center justify-center mx-auto mb-3">
                <Zap className="w-6 h-6 text-[var(--muted-foreground)]" />
              </div>
              <p className="text-sm text-[var(--muted-foreground)]">Select a skill to view details</p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
