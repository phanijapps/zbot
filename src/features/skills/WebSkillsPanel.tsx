// ============================================================================
// WEB SKILLS PANEL
// Skill management for web dashboard (uses transport layer)
// ============================================================================

import { useState, useEffect } from "react";
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
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-violet-500" />
      </div>
    );
  }

  return (
    <div className="flex h-full">
      {/* Skills List */}
      <div className="w-80 border-r border-gray-800 flex flex-col">
        <div className="p-4 border-b border-gray-800 flex items-center justify-between">
          <h1 className="text-lg font-bold">Skills</h1>
          <button
            onClick={() => setIsCreating(true)}
            className="bg-violet-600 hover:bg-violet-700 text-white px-3 py-1 rounded text-sm transition-colors"
          >
            New
          </button>
        </div>

        {error && (
          <div className="p-3 bg-red-900/30 border-b border-red-800 text-red-200 text-sm">
            {error}
            <button onClick={() => setError(null)} className="ml-2 text-red-400 hover:text-red-300">
              Dismiss
            </button>
          </div>
        )}

        <div className="flex-1 overflow-auto">
          {skills.length === 0 ? (
            <div className="p-4 text-center text-gray-500">
              <p>No skills yet</p>
              <p className="text-sm mt-1">Create your first skill</p>
            </div>
          ) : (
            Object.entries(skillsByCategory).map(([category, categorySkills]) => (
              <div key={category}>
                <div className="px-4 py-2 text-xs text-gray-500 uppercase tracking-wide bg-[#0a0a0a]">
                  {category}
                </div>
                {categorySkills.map((skill) => (
                  <button
                    key={skill.id}
                    onClick={() => setSelectedSkill(skill)}
                    className={`w-full text-left px-4 py-3 border-b border-gray-800 hover:bg-gray-800/50 transition-colors ${
                      selectedSkill?.id === skill.id ? "bg-violet-500/10 border-l-2 border-l-violet-500" : ""
                    }`}
                  >
                    <div className="font-medium">{skill.displayName}</div>
                    <div className="text-sm text-gray-500 truncate">{skill.description}</div>
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
          <div className="p-6">
            <h2 className="text-xl font-bold mb-4">Create New Skill</h2>

            <div className="space-y-4 max-w-2xl">
              <div>
                <label className="block text-sm text-gray-400 mb-1">Name (ID)</label>
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
                  className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 focus:outline-none focus:border-violet-500"
                />
              </div>

              <div>
                <label className="block text-sm text-gray-400 mb-1">Display Name</label>
                <input
                  type="text"
                  value={newSkill.displayName}
                  onChange={(e) => setNewSkill({ ...newSkill, displayName: e.target.value })}
                  placeholder="My Skill"
                  className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 focus:outline-none focus:border-violet-500"
                />
              </div>

              <div>
                <label className="block text-sm text-gray-400 mb-1">Description</label>
                <input
                  type="text"
                  value={newSkill.description}
                  onChange={(e) => setNewSkill({ ...newSkill, description: e.target.value })}
                  placeholder="What does this skill do?"
                  className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 focus:outline-none focus:border-violet-500"
                />
              </div>

              <div>
                <label className="block text-sm text-gray-400 mb-1">Category</label>
                <input
                  type="text"
                  value={newSkill.category}
                  onChange={(e) => setNewSkill({ ...newSkill, category: e.target.value })}
                  placeholder="general"
                  className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 focus:outline-none focus:border-violet-500"
                />
              </div>

              <div>
                <label className="block text-sm text-gray-400 mb-1">Instructions</label>
                <textarea
                  value={newSkill.instructions}
                  onChange={(e) => setNewSkill({ ...newSkill, instructions: e.target.value })}
                  placeholder="Instructions for the agent when using this skill..."
                  rows={10}
                  className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 focus:outline-none focus:border-violet-500 resize-none font-mono text-sm"
                />
              </div>

              <div className="flex gap-3">
                <button
                  onClick={() => setIsCreating(false)}
                  className="px-4 py-2 text-gray-400 hover:text-white transition-colors"
                >
                  Cancel
                </button>
                <button
                  onClick={handleCreateSkill}
                  disabled={!newSkill.name}
                  className="bg-violet-600 hover:bg-violet-700 disabled:opacity-50 text-white px-4 py-2 rounded-lg transition-colors"
                >
                  Create Skill
                </button>
              </div>
            </div>
          </div>
        ) : selectedSkill ? (
          <div className="p-6">
            <div className="flex items-start justify-between mb-6">
              <div>
                <h2 className="text-xl font-bold">{selectedSkill.displayName}</h2>
                <p className="text-gray-500">{selectedSkill.id}</p>
              </div>
              <button
                onClick={() => handleDeleteSkill(selectedSkill.id)}
                className="text-gray-500 hover:text-red-400 transition-colors"
              >
                Delete
              </button>
            </div>

            <div className="space-y-4 max-w-2xl">
              <div>
                <label className="block text-sm text-gray-500 mb-1">Description</label>
                <p className="text-gray-300">{selectedSkill.description || "No description"}</p>
              </div>

              <div>
                <label className="block text-sm text-gray-500 mb-1">Category</label>
                <p className="text-gray-300">{selectedSkill.category}</p>
              </div>

              <div>
                <label className="block text-sm text-gray-500 mb-1">Instructions</label>
                <pre className="bg-gray-900 rounded-lg p-4 text-sm text-gray-300 whitespace-pre-wrap font-mono overflow-auto max-h-96">
                  {selectedSkill.instructions}
                </pre>
              </div>
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-center h-full text-gray-500">
            <p>Select a skill to view details</p>
          </div>
        )}
      </div>
    </div>
  );
}
