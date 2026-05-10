use gateway_services::skills::{SkillFrontmatter, WardSetup};
use std::path::Path;

/// Read `ward_setup` from specific skills' SKILL.md files.
///
/// Only reads skills in `skill_names` — prevents life-os dirs in coding wards, etc.
pub fn collect_ward_setups_for_skills(skills_dir: &Path, skill_names: &[String]) -> Vec<WardSetup> {
    let mut setups = Vec::new();
    for name in skill_names {
        setups.extend(collect_ward_setup_for_skill(skills_dir, name));
    }
    setups
}

/// Read `ward_setup` from a single skill's SKILL.md.
pub fn collect_ward_setup_for_skill(skills_dir: &Path, skill_name: &str) -> Vec<WardSetup> {
    let skill_md = skills_dir.join(skill_name).join("SKILL.md");
    if !skill_md.exists() {
        return vec![];
    }
    let content = match std::fs::read_to_string(&skill_md) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let yaml = match extract_yaml_frontmatter(&content) {
        Some(y) => y,
        None => return vec![],
    };
    match serde_yaml::from_str::<SkillFrontmatter>(yaml) {
        Ok(fm) => fm.ward_setup.into_iter().collect(),
        Err(_) => vec![],
    }
}

/// Extract the YAML frontmatter block from a `---`-delimited document.
///
/// Returns the trimmed content between the first pair of `---` markers,
/// or `None` if the document doesn't start with `---`.
fn extract_yaml_frontmatter(content: &str) -> Option<&str> {
    let content = content.trim_start();
    if !content.starts_with("---") {
        return None;
    }
    let after_first = &content[3..];
    let end = after_first.find("\n---")?;
    Some(after_first[..end].trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_extracted_from_valid_doc() {
        let doc = "---\nname: test\n---\nbody";
        assert_eq!(extract_yaml_frontmatter(doc), Some("name: test"));
    }

    #[test]
    fn frontmatter_returns_none_when_no_delimiter() {
        assert!(extract_yaml_frontmatter("no frontmatter here").is_none());
    }

    #[test]
    fn collect_for_unknown_skill_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let result = collect_ward_setup_for_skill(dir.path(), "nonexistent");
        assert!(result.is_empty());
    }
}
