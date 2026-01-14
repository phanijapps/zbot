// ============================================================================
// SKILL.MD FORM
// Form-based editor for SKILL.md with frontmatter + instructions
// ============================================================================

import { useEffect } from "react";
import { Sparkles, Lock } from "lucide-react";
import { Input } from "@/shared/ui/input";
import { Label } from "@/shared/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/shared/ui/select";
import { Textarea } from "@/shared/ui/textarea";

interface SkillMdFormProps {
  // Read-only ID (name)
  name: string;
  isNewSkill: boolean;

  // Editable fields
  displayName: string;
  description: string;
  category: string;
  instructions: string;

  // Available categories
  categories: string[];

  // Callbacks
  onDisplayNameChange: (value: string) => void;
  onDescriptionChange: (value: string) => void;
  onCategoryChange: (value: string) => void;
  onInstructionsChange: (value: string) => void;
  onSave: () => void;
}

const SKILL_CATEGORIES = [
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

export function SkillMdForm({
  name,
  isNewSkill,
  displayName,
  description,
  category,
  instructions,
  // categories,
  onDisplayNameChange,
  onDescriptionChange,
  onCategoryChange,
  onInstructionsChange,
  onSave,
}: SkillMdFormProps) {
  // Auto-save on any change
  useEffect(() => {
    const timer = setTimeout(() => {
      onSave();
    }, 500);
    return () => clearTimeout(timer);
  }, [displayName, description, category, instructions, onSave]);

  return (
    <div className="flex-1 overflow-y-auto p-6">
      {/* Header */}
      <div className="flex items-center gap-3 mb-6 pb-4 border-b border-white/10">
        <div className="p-2 rounded-lg bg-yellow-500/20">
          <Lock className="size-5 text-yellow-400" />
        </div>
        <div>
          <h2 className="text-lg font-semibold text-white">Skill Configuration</h2>
          <p className="text-sm text-gray-400">
            Changes are automatically saved to SKILL.md
          </p>
        </div>
      </div>

      <div className="space-y-4 max-w-4xl">
        {/* Name (ID) and Display Name - side by side */}
        <div className="grid grid-cols-2 gap-4">
          <div>
            <Label className="text-gray-400 text-xs mb-1.5 flex items-center gap-2">
              <Sparkles className="size-3.5 text-blue-400" />
              Name (ID) {isNewSkill && <span className="text-gray-500 font-normal">(auto-generated)</span>}
            </Label>
            <Input
              value={name || "(auto-generated from Display Name)"}
              disabled
              className="bg-white/5 border-white/10 text-gray-500 cursor-not-allowed h-9 text-sm"
            />
          </div>
          <div>
            <Label className="text-gray-400 text-xs mb-1.5 block">Display Name</Label>
            <Input
              placeholder="My Skill"
              value={displayName}
              onChange={(e) => onDisplayNameChange(e.target.value)}
              className="bg-white/5 border-white/10 text-white h-9 text-sm"
            />
          </div>
        </div>

        {/* Description and Category - side by side */}
        <div className="grid grid-cols-2 gap-4">
          <div>
            <Label className="text-gray-400 text-xs mb-1.5 block">Description</Label>
            <Input
              placeholder="What does this skill do?"
              value={description}
              onChange={(e) => onDescriptionChange(e.target.value)}
              className="bg-white/5 border-white/10 text-white h-9 text-sm"
            />
          </div>
          <div>
            <Label className="text-gray-400 text-xs mb-1.5 block">Category</Label>
            <Select value={category} onValueChange={onCategoryChange}>
              <SelectTrigger className="bg-white/5 border-white/10 text-white h-9 text-sm">
                <SelectValue placeholder="Select category" />
              </SelectTrigger>
              <SelectContent>
                {SKILL_CATEGORIES.map((cat) => (
                  <SelectItem key={cat} value={cat}>
                    {cat.charAt(0).toUpperCase() + cat.slice(1)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        {/* Instructions - full width textarea */}
        <div>
          <Label className="text-gray-400 text-xs mb-1.5 flex items-center justify-between">
            <span>Instructions (Markdown)</span>
            <span className="text-gray-500 text-xs">Auto-saves as you type</span>
          </Label>
          <Textarea
            value={instructions}
            onChange={(e) => onInstructionsChange(e.target.value)}
            placeholder="You are a helpful skill that..."
            className="flex-1 bg-white/5 border-white/10 text-white font-mono text-sm resize-y min-h-[300px]"
            spellCheck={false}
          />
        </div>

        {/* Info Box */}
        <div className="bg-blue-500/10 border border-blue-500/20 rounded-lg p-3">
          <p className="text-xs text-blue-300">
            <strong>SKILL.md</strong> stores skill metadata (name, displayName, description, category) in frontmatter and instructions below.
            Additional files can be added to the <strong>assets/</strong>, <strong>resources/</strong>, and <strong>scripts/</strong> folders.
          </p>
        </div>
      </div>
    </div>
  );
}
