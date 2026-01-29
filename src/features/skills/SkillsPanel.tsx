// ============================================================================
// SKILLS FEATURE
// Agent skills and plugins management
// ============================================================================

import { useState, useEffect } from "react";
import { Plus, Sparkles, Trash2, Loader2, RefreshCw, Edit } from "lucide-react";
import { Button } from "@/shared/ui/button";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/shared/ui/card";
import { Badge } from "@/shared/ui/badge";
import { SkillIDEPage } from "./SkillIDEPage";
import * as skillsService from "@/services/skills";
import type { Skill } from "@/shared/types";
import { useVaults } from "@/features/vaults/useVaults";

const SKILL_CATEGORIES = [
  "education",
  "utility",
  "coding",
  "writing",
  "analysis",
  "communication",
  "productivity",
  "research",
  "creative",
  "automation",
  "other",
];

export function SkillsPanel() {
  const { currentVault } = useVaults();
  const [skills, setSkills] = useState<Skill[]>([]);
  const [loading, setLoading] = useState(true);
  const [showFullPageEditor, setShowFullPageEditor] = useState(false);
  const [editingSkill, setEditingSkill] = useState<Skill | null>(null);
  const [selectedCategory, setSelectedCategory] = useState<string>("all");
  const [refreshing, setRefreshing] = useState(false);

  // Load skills on mount and when vault changes
  useEffect(() => {
    loadSkills();
  }, [currentVault?.id]); // Reload when vault changes

  const loadSkills = async () => {
    setLoading(true);
    try {
      const loaded = await skillsService.listSkills();
      setSkills(loaded);
    } catch (error) {
      console.error("Failed to load skills:", error);
    } finally {
      setLoading(false);
    }
  };

  const handleRefresh = async () => {
    setRefreshing(true);
    await loadSkills();
    setRefreshing(false);
  };

  const handleOpenCreateEditor = () => {
    setEditingSkill(null);
    setShowFullPageEditor(true);
  };

  const handleOpenEditEditor = (skill: Skill) => {
    setEditingSkill(skill);
    setShowFullPageEditor(true);
  };

  const handleSaveSkill = async (_skill: Omit<Skill, "id" | "createdAt">) => {
    await loadSkills();
  };

  const handleDeleteSkill = async (id: string) => {
    if (confirm("Are you sure you want to delete this skill?")) {
      try {
        await skillsService.deleteSkill(id);
        await loadSkills();
      } catch (error) {
        console.error("Failed to delete skill:", error);
      }
    }
  };

  const categories = ["all", ...SKILL_CATEGORIES];
  const filteredSkills = selectedCategory === "all"
    ? skills
    : skills.filter((s) => s.category === selectedCategory);

  // Get gradient based on category
  const getCategoryGradient = (category: string) => {
    const gradients: Record<string, string> = {
      "education": "from-amber-500 to-yellow-600",
      "coding": "from-blue-500 to-purple-600",
      "analysis": "from-green-500 to-teal-600",
      "automation": "from-orange-500 to-red-600",
      "utility": "from-yellow-500 to-orange-600",
      "communication": "from-pink-500 to-rose-600",
      "research": "from-indigo-500 to-blue-600",
      "writing": "from-cyan-500 to-blue-600",
      "productivity": "from-violet-500 to-purple-600",
      "creative": "from-gray-500 to-slate-600",
      "other": "from-emerald-500 to-green-600",
    };
    return gradients[category] || "from-purple-500 to-pink-600";
  };

  return (
    <>
      <div className="p-6">
        <div className="flex items-center justify-between mb-6">
          <div>
            <h2 className="text-2xl font-bold text-foreground">Skills</h2>
            <p className="text-muted-foreground text-sm mt-1">
              Extend agent capabilities with skills following the Agent Skills specification
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              className="border-border text-foreground hover:bg-accent"
              onClick={handleRefresh}
              disabled={refreshing}
            >
              <RefreshCw className={`size-4 ${refreshing ? "animate-spin" : ""}`} />
            </Button>
            <Button
              className="bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700 text-white gap-2"
              onClick={handleOpenCreateEditor}
            >
              <Plus className="size-4" />
              Add Skill
            </Button>
          </div>
        </div>

        {/* Category Filter */}
        <div className="mb-6 flex flex-wrap gap-2">
          {categories.map((cat) => (
            <button
              key={cat}
              onClick={() => setSelectedCategory(cat)}
              className={`px-4 py-2 rounded-lg text-sm font-medium transition-all ${
                selectedCategory === cat
                  ? "bg-primary text-primary-foreground"
                  : "bg-muted text-foreground hover:bg-accent"
              }`}
            >
              {cat === "all" ? "All Skills" : cat}
            </button>
          ))}
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-20">
            <Loader2 className="size-8 text-foreground animate-spin" />
          </div>
        ) : filteredSkills.length === 0 ? (
          <Card className="bg-card border-border">
            <CardContent className="py-16 text-center">
              <Sparkles className="size-12 text-muted-foreground mx-auto mb-4" />
              <h3 className="text-foreground text-lg font-medium mb-2">No Skills Found</h3>
              <p className="text-muted-foreground text-sm mb-4">
                {selectedCategory === "all"
                  ? "Get started by adding your first skill"
                  : `No skills in ${selectedCategory} category`}
              </p>
              <Button
                onClick={handleOpenCreateEditor}
                className="bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700 text-white"
              >
                <Plus className="size-4 mr-2" />
                Add Your First Skill
              </Button>
            </CardContent>
          </Card>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {filteredSkills.map((skill) => (
              <Card
                key={skill.id}
                className="bg-card border-border hover:bg-accent transition-colors group"
              >
                <CardHeader>
                  <div className="flex items-start justify-between">
                    <div className={`bg-gradient-to-br ${getCategoryGradient(skill.category)} p-3 rounded-xl`}>
                      <Sparkles className="size-5 text-white" />
                    </div>
                    <Badge variant="secondary" className="bg-muted text-foreground text-xs">
                      {skill.category}
                    </Badge>
                  </div>
                </CardHeader>
                <CardContent>
                  <CardTitle className="text-foreground text-lg mb-2">
                    {skill.displayName}
                  </CardTitle>
                  <CardDescription className="text-muted-foreground text-sm mb-4 line-clamp-2">
                    {skill.description}
                  </CardDescription>
                  <div className="flex items-center justify-between">
                    <code className="text-xs text-purple-300 bg-purple-500/10 px-2 py-1 rounded">
                      {skill.name}
                    </code>
                    <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleOpenEditEditor(skill)}
                        className="text-muted-foreground hover:text-foreground h-8 w-8 p-0"
                      >
                        <Edit className="size-4" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleDeleteSkill(skill.id)}
                        className="text-muted-foreground hover:text-destructive h-8 w-8 p-0"
                      >
                        <Trash2 className="size-4" />
                      </Button>
                    </div>
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        )}

        {/* Info Box */}
        <Card className="mt-6 bg-blue-500/10 border-blue-500/20">
          <CardContent className="p-4">
            <p className="text-sm text-blue-200">
              <strong>About Skills:</strong> Skills follow the{" "}
              <a
                href="https://agentskills.io/specification"
                target="_blank"
                rel="noopener noreferrer"
                className="underline hover:text-blue-100"
              >
                Agent Skills specification
              </a>
              {" "}with SKILL.md files containing YAML frontmatter and markdown instructions. Skills are stored in{" "}
              <code className="bg-white/10 px-1.5 py-0.5 rounded text-blue-200">
                {currentVault?.path || "~/.config/zeroagent"}/skills/
              </code>
            </p>
          </CardContent>
        </Card>
      </div>

      {showFullPageEditor && (
        <SkillIDEPage
          onClose={() => setShowFullPageEditor(false)}
          onSave={handleSaveSkill}
          initialSkill={editingSkill}
        />
      )}
    </>
  );
}
