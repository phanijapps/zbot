import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { SkillResponse } from "@/services/transport";

interface SkillsStepProps {
  enabledSkillIds: string[];
  onChange: (ids: string[]) => void;
}

export function SkillsStep({ enabledSkillIds, onChange }: SkillsStepProps) {
  const [skills, setSkills] = useState<SkillResponse[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const load = async () => {
      try {
        const transport = await getTransport();
        const result = await transport.listSkills();
        if (result.success && result.data) {
          setSkills(result.data);
          // Select all skills by default on first load
          if (enabledSkillIds.length === 0) {
            onChange(result.data.map((s) => s.id));
          }
        }
      } finally {
        setIsLoading(false);
      }
    };
    load();
  }, []);

  if (isLoading) {
    return <div className="settings-loading"><Loader2 className="loading-spinner__icon" /></div>;
  }

  const byCategory = skills.reduce<Record<string, SkillResponse[]>>((acc, skill) => {
    const cat = skill.category || "other";
    (acc[cat] = acc[cat] || []).push(skill);
    return acc;
  }, {});

  const toggleSkill = (id: string) => {
    if (enabledSkillIds.includes(id)) {
      onChange(enabledSkillIds.filter((s) => s !== id));
    } else {
      onChange([...enabledSkillIds, id]);
    }
  };

  const toggleCategory = (categorySkills: SkillResponse[]) => {
    const ids = categorySkills.map((s) => s.id);
    const allSelected = ids.every((id) => enabledSkillIds.includes(id));
    if (allSelected) {
      onChange(enabledSkillIds.filter((id) => !ids.includes(id)));
    } else {
      onChange([...new Set([...enabledSkillIds, ...ids])]);
    }
  };

  return (
    <div>
      {Object.entries(byCategory).map(([category, categorySkills]) => {
        const allSelected = categorySkills.every((s) => enabledSkillIds.includes(s.id));
        return (
          <div key={category} className="skill-category">
            <div className="skill-category__header">
              <span className="skill-category__name">{category}</span>
              <button
                className="skill-category__toggle"
                onClick={() => toggleCategory(categorySkills)}
              >
                {allSelected ? "Deselect all" : "Select all"}
              </button>
            </div>
            {categorySkills.map((skill) => (
              <div
                key={skill.id}
                className={`skill-toggle ${enabledSkillIds.includes(skill.id) ? "skill-toggle--on" : ""}`}
                onClick={() => toggleSkill(skill.id)}
              >
                <div className="skill-toggle__info">
                  <div className="skill-toggle__name">{skill.displayName || skill.name}</div>
                  <div className="skill-toggle__desc">{skill.description}</div>
                </div>
              </div>
            ))}
          </div>
        );
      })}
      {skills.length === 0 && (
        <p className="settings-hint">No skills installed. You can add skills later.</p>
      )}
    </div>
  );
}
