// ============================================================================
// SKILLS SERVICE
// Frontend service for skill management
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import type { Skill } from "@/shared/types";

/**
 * List all available skills
 */
export async function listSkills(): Promise<Skill[]> {
  return invoke("list_skills");
}

/**
 * Get a single skill by ID
 */
export async function getSkill(id: string): Promise<Skill> {
  return invoke("get_skill", { id });
}

/**
 * Create a new skill
 */
export async function createSkill(skill: Omit<Skill, "id" | "createdAt">): Promise<Skill> {
  const skillWithId: Skill = {
    ...skill,
    id: skill.name, // Use name as ID
    createdAt: new Date().toISOString(),
  };
  return invoke("create_skill", { skill: skillWithId });
}

/**
 * Update an existing skill
 */
export async function updateSkill(id: string, skill: Omit<Skill, "id" | "createdAt">): Promise<Skill> {
  const skillWithId: Skill = {
    ...skill,
    id: skill.name, // Update ID if name changed
    createdAt: new Date().toISOString(),
  };
  return invoke("update_skill", { id, skill: skillWithId });
}

/**
 * Delete a skill
 */
export async function deleteSkill(id: string): Promise<void> {
  return invoke("delete_skill", { id });
}

/**
 * Validate skill name (lowercase, numbers, hyphens only, doesn't start/end with hyphen)
 */
export function validateSkillName(name: string): { valid: boolean; error?: string } {
  // Check length
  if (name.length === 0) {
    return { valid: false, error: "Name is required" };
  }
  if (name.length > 64) {
    return { valid: false, error: "Name must be 64 characters or less" };
  }

  // Check for valid characters (lowercase letters, numbers, hyphens)
  const validNameRegex = /^[a-z0-9-]+$/;
  if (!validNameRegex.test(name)) {
    return { valid: false, error: "Name can only contain lowercase letters, numbers, and hyphens" };
  }

  // Check for consecutive hyphens
  if (name.includes("--")) {
    return { valid: false, error: "Name cannot contain consecutive hyphens" };
  }

  // Check for leading/trailing hyphens
  if (name.startsWith("-") || name.endsWith("-")) {
    return { valid: false, error: "Name cannot start or end with a hyphen" };
  }

  return { valid: true };
}

/**
 * Sanitize a name to be valid as a skill name
 */
export function sanitizeSkillName(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9-]/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
}

/**
 * Format a skill name for display (convert kebab-case to Title Case)
 */
export function formatDisplayName(name: string): string {
  return name
    .split("-")
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

// ============================================================================
// Skill File Operations
// ============================================================================

/** Skill file entry */
export interface SkillFile {
  name: string;
  path: string;
  isFile: boolean;
  isBinary: boolean;
  isProtected: boolean;
  size: number;
}

/** Skill file content */
export interface SkillFileContent {
  content: string;
  isBinary: boolean;
  isMarkdown: boolean;
}

/**
 * List files in a skill folder
 */
export async function listSkillFiles(skillId: string): Promise<SkillFile[]> {
  return invoke("list_skill_files", { skillId });
}

/**
 * Read a file's content from a skill folder
 */
export async function readSkillFile(skillId: string, filePath: string): Promise<SkillFileContent> {
  return invoke("read_skill_file", { skillId, filePath });
}

/**
 * Write or create a file in a skill folder
 */
export async function writeSkillFile(skillId: string, filePath: string, content: string): Promise<void> {
  return invoke("write_skill_file", { skillId, filePath, content });
}

/**
 * Create a folder in a skill directory
 */
export async function createSkillFolder(skillId: string, folderPath: string): Promise<void> {
  return invoke("create_skill_folder", { skillId, folderPath });
}

/**
 * Delete a file or folder from a skill directory
 */
export async function deleteSkillFile(skillId: string, filePath: string): Promise<void> {
  return invoke("delete_skill_file", { skillId, filePath });
}
