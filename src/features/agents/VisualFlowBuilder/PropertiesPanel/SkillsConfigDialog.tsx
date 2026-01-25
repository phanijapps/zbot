// ============================================================================
// ZERO IDE - SKILLS CONFIGURATION DIALOG
// Modal dialog for configuring skill selections
// ============================================================================

import { memo, useState, useEffect, useCallback } from "react";
import type { Skill } from "../hooks/useAgentResources";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/shared/ui/dialog";
import { Button } from "@/shared/ui/button";

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const CheckIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2.5" viewBox="0 0 24 24">
    <path d="M20 6 9 17l-5-5" />
  </svg>
);

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface SkillsConfigDialogProps {
  open: boolean;
  onClose: () => void;
  onSave: (skills: string[]) => void;
  availableSkills: Skill[];
  initialSkills: string[];
}

// -----------------------------------------------------------------------------
// Skill Item Component
// -----------------------------------------------------------------------------

interface SkillItemProps {
  skill: Skill;
  isSelected: boolean;
  onToggle: () => void;
}

const SkillItem = memo(({ skill, isSelected, onToggle }: SkillItemProps) => {
  return (
    <div
      className={`flex items-center gap-3 p-3 rounded-lg cursor-pointer transition-colors ${
        isSelected
          ? "bg-green-500/10 border border-green-500/30"
          : "bg-white/5 hover:bg-white/10 border border-transparent"
      }`}
      onClick={onToggle}
    >
      <input
        type="checkbox"
        checked={isSelected}
        onChange={onToggle}
        className="rounded"
        onClick={(e) => e.stopPropagation()}
      />
      {isSelected && <CheckIcon />}
      <span className="text-xl">📚</span>
      <div className="flex-1 min-w-0">
        <p className="text-sm text-white font-medium">{skill.display_name || skill.name}</p>
        <p className="text-xs text-gray-500 truncate">{skill.description}</p>
        {skill.category && (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-white/10 text-gray-400 inline-block mt-1">
            {skill.category}
          </span>
        )}
      </div>
    </div>
  );
});

SkillItem.displayName = "SkillItem";

// -----------------------------------------------------------------------------
// Main Dialog Component
// -----------------------------------------------------------------------------

export const SkillsConfigDialog = memo(({
  open,
  onClose,
  onSave,
  availableSkills,
  initialSkills,
}: SkillsConfigDialogProps) => {
  const [selectedSkills, setSelectedSkills] = useState<Set<string>>(new Set());

  // Initialize state when dialog opens - default to empty (no skills selected)
  useEffect(() => {
    if (open) {
      // Use initialSkills if provided, otherwise default to empty set
      setSelectedSkills(new Set(initialSkills));
    }
  }, [open, initialSkills]);

  const handleToggle = useCallback((skillId: string) => {
    setSelectedSkills((prev) => {
      const next = new Set(prev);
      if (next.has(skillId)) {
        next.delete(skillId);
      } else {
        next.add(skillId);
      }
      return next;
    });
  }, []);

  const handleSave = useCallback(() => {
    onSave(Array.from(selectedSkills));
    onClose();
  }, [selectedSkills, onSave, onClose]);

  const handleSelectAll = useCallback(() => {
    setSelectedSkills(new Set(availableSkills.map((s) => s.id || s.name)));
  }, [availableSkills]);

  const handleClearAll = useCallback(() => {
    setSelectedSkills(new Set());
  }, []);

  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="bg-[#141414] border-white/10 text-white max-w-lg max-h-[80vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle className="text-lg font-semibold">Configure Skills</DialogTitle>
          <p className="text-sm text-gray-400">Select which skills this agent can use</p>
        </DialogHeader>

        <div className="flex gap-2 mb-4">
          <Button
            variant="outline"
            size="sm"
            onClick={handleSelectAll}
            className="border-white/20 text-white hover:bg-white/5 text-xs"
          >
            Select All
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={handleClearAll}
            className="border-white/20 text-white hover:bg-white/5 text-xs"
          >
            Clear All
          </Button>
        </div>

        <div className="flex-1 overflow-y-auto space-y-2 pr-2">
          {availableSkills.length === 0 ? (
            <div className="text-center py-8">
              <p className="text-sm text-gray-500">No skills available</p>
              <p className="text-xs text-gray-600 mt-1">Add skills in the Vault to see them here</p>
            </div>
          ) : (
            availableSkills.map((skill) => {
              const skillId = skill.id || skill.name;
              return (
                <SkillItem
                  key={skillId}
                  skill={skill}
                  isSelected={selectedSkills.has(skillId)}
                  onToggle={() => handleToggle(skillId)}
                />
              );
            })
          )}
        </div>

        <div className="flex items-center justify-between pt-4 border-t border-white/10 mt-4">
          <p className="text-xs text-gray-400">
            {selectedSkills.size} of {availableSkills.length} skills selected
          </p>
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={onClose}
              className="border-white/20 text-white hover:bg-white/5"
            >
              Cancel
            </Button>
            <Button
              size="sm"
              onClick={handleSave}
              className="bg-green-600 hover:bg-green-700"
            >
              Save
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
});

SkillsConfigDialog.displayName = "SkillsConfigDialog";
