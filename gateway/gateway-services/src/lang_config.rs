//! # Language Config
//!
//! Language-specific signature extraction patterns loaded from YAML config files.
//!
//! Config files live in `~/Documents/zbot/config/wards/*.yaml` and describe
//! how to extract function/class signatures and docstrings for a given language.
//! This replaces hardcoded patterns in core module indexing.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Language-specific configuration for signature extraction.
///
/// Each YAML file in `config/wards/` describes one language: its file extensions,
/// regex patterns for locating function/class signatures, and optional docstring
/// and convention metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LangConfig {
    /// Human-readable language name (e.g. `"python"`, `"rust"`)
    pub language: String,

    /// File extensions this config applies to (e.g. `["py"]`, `["rs"]`)
    pub file_extensions: Vec<String>,

    /// Named regex patterns for extracting signatures.
    ///
    /// Typical keys: `"function"`, `"class"`. Values are regex strings where
    /// capture group 1 is the signature text to emit.
    pub signature_patterns: HashMap<String, String>,

    /// Optional regex for extracting the first docstring from a file.
    #[serde(default)]
    pub docstring_pattern: Option<String>,

    /// Optional language conventions (informational, used by downstream tasks).
    #[serde(default)]
    pub conventions: Vec<String>,
}

impl LangConfig {
    /// Find the first config whose `file_extensions` contains `ext`.
    ///
    /// `ext` should be the bare extension without a leading dot (e.g. `"py"`).
    pub fn find_for_extension<'a>(configs: &'a [LangConfig], ext: &str) -> Option<&'a LangConfig> {
        configs
            .iter()
            .find(|c| c.file_extensions.iter().any(|e| e == ext))
    }

    /// Compile this config into a [`CompiledLangConfig`] with pre-built regexes.
    ///
    /// Patterns that fail to compile are skipped with a warning. The returned
    /// `CompiledLangConfig` is the runtime type used for extraction.
    pub fn compile(&self) -> CompiledLangConfig {
        let compiled_patterns = self
            .signature_patterns
            .iter()
            .filter_map(|(kind, pattern_str)| {
                match Regex::new(pattern_str) {
                    Ok(re) => Some((kind.clone(), re)),
                    Err(e) => {
                        tracing::warn!(
                            "Invalid signature pattern for '{}' in language '{}': {}",
                            kind,
                            self.language,
                            e
                        );
                        None
                    }
                }
            })
            .collect();

        let compiled_docstring = self.docstring_pattern.as_deref().and_then(|pattern_str| {
            match Regex::new(pattern_str) {
                Ok(re) => Some(re),
                Err(e) => {
                    tracing::warn!(
                        "Invalid docstring pattern for language '{}': {}",
                        self.language,
                        e
                    );
                    None
                }
            }
        });

        CompiledLangConfig {
            language: self.language.clone(),
            file_extensions: self.file_extensions.clone(),
            compiled_patterns,
            compiled_docstring,
            conventions: self.conventions.clone(),
        }
    }
}

/// Compiled version of [`LangConfig`] with pre-built regexes.
///
/// Created via [`LangConfig::compile`] or [`compile_all`]. Holds the same
/// metadata as `LangConfig` plus pre-compiled `Regex` objects so patterns are
/// not recompiled on every file scan.
///
/// `Regex` does not implement `Clone` or `Serialize`/`Deserialize`, so this
/// type cannot derive those traits. Use `LangConfig` for serialization.
pub struct CompiledLangConfig {
    /// Human-readable language name (e.g. `"python"`, `"rust"`)
    pub language: String,

    /// File extensions this config applies to (e.g. `["py"]`, `["rs"]`)
    pub file_extensions: Vec<String>,

    /// Pre-compiled signature patterns: `(kind, regex)` pairs.
    compiled_patterns: Vec<(String, Regex)>,

    /// Pre-compiled docstring pattern, if any.
    compiled_docstring: Option<Regex>,

    /// Optional language conventions (informational).
    pub conventions: Vec<String>,
}

impl CompiledLangConfig {
    /// Find the first compiled config whose `file_extensions` contains `ext`.
    ///
    /// `ext` should be the bare extension without a leading dot (e.g. `"py"`).
    pub fn find_for_extension<'a>(
        configs: &'a [CompiledLangConfig],
        ext: &str,
    ) -> Option<&'a CompiledLangConfig> {
        configs
            .iter()
            .find(|c| c.file_extensions.iter().any(|e| e == ext))
    }

    /// Extract all function/class signatures from `file_path` using this config's
    /// pre-compiled `signature_patterns`.
    ///
    /// Each pattern is applied to the full file content. For every match, capture
    /// group 1 is collected and trimmed. Returns an empty vec when the file cannot
    /// be read or no patterns match.
    pub fn extract_signatures(&self, file_path: &Path) -> Vec<String> {
        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to read {:?} for signature extraction: {}", file_path, e);
                return vec![];
            }
        };

        let mut signatures = Vec::new();

        for (_kind, re) in &self.compiled_patterns {
            for caps in re.captures_iter(&content) {
                if let Some(m) = caps.get(1) {
                    let sig = m.as_str().trim().to_string();
                    if !sig.is_empty() {
                        signatures.push(sig);
                    }
                }
            }
        }

        signatures
    }

    /// Extract the first docstring from `file_path` using the pre-compiled
    /// `docstring_pattern`.
    ///
    /// Returns `None` when there is no pattern configured, the file cannot be
    /// read, or the pattern does not match. Capture group 1 is returned.
    pub fn extract_first_docstring(&self, file_path: &Path) -> Option<String> {
        let re = self.compiled_docstring.as_ref()?;

        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to read {:?} for docstring extraction: {}", file_path, e);
                return None;
            }
        };

        re.captures(&content)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().trim().to_string())
    }
}

/// Compile all configs in `configs` into [`CompiledLangConfig`] instances.
///
/// This is the standard bridge between the deserialization layer (`LangConfig`)
/// and the runtime extraction layer (`CompiledLangConfig`). Call once after
/// loading configs, then reuse the compiled slice across all file scans.
pub fn compile_all(configs: &[LangConfig]) -> Vec<CompiledLangConfig> {
    configs.iter().map(|c| c.compile()).collect()
}

/// Load a single language config from a YAML file.
///
/// Returns an error string if the file cannot be read or parsed.
pub fn load_lang_config(path: &Path) -> Result<LangConfig, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read lang config {:?}: {}", path, e))?;

    serde_yaml::from_str(&content)
        .map_err(|e| format!("Failed to parse lang config {:?}: {}", path, e))
}

/// Load all language configs from `dir`.
///
/// - If `dir` does not exist, returns an empty vec (graceful — first-run scenario).
/// - Files that fail to parse are skipped with a warning.
/// - Only files with `.yaml` or `.yml` extensions are considered.
pub fn load_all_lang_configs(dir: &Path) -> Result<Vec<LangConfig>, String> {
    if !dir.exists() {
        tracing::debug!("Lang config directory does not exist, returning empty: {:?}", dir);
        return Ok(vec![]);
    }

    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read lang config directory {:?}: {}", dir, e))?;

    let mut configs = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "yaml" && ext != "yml" {
            continue;
        }

        match load_lang_config(&path) {
            Ok(config) => configs.push(config),
            Err(e) => {
                tracing::warn!("Skipping invalid lang config {:?}: {}", path, e);
            }
        }
    }

    Ok(configs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    fn python_yaml() -> &'static str {
        r#"
language: python
file_extensions:
  - py
signature_patterns:
  function: "(?m)^(def \\w+\\([^)]*\\))"
  class: "(?m)^(class \\w+[^:]*):"
docstring_pattern: "(?s)\"\"\"(.*?)\"\"\""
conventions:
  - "snake_case functions"
  - "PascalCase classes"
"#
    }

    fn write_yaml(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    fn write_python_file(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    // -------------------------------------------------------------------------
    // Parsing
    // -------------------------------------------------------------------------

    #[test]
    fn test_parse_python_config_yaml() {
        let dir = tempdir().unwrap();
        let path = write_yaml(dir.path(), "python.yaml", python_yaml());

        let config = load_lang_config(&path).unwrap();

        assert_eq!(config.language, "python");
        assert_eq!(config.file_extensions, vec!["py"]);
        assert!(config.signature_patterns.contains_key("function"));
        assert!(config.signature_patterns.contains_key("class"));
        assert!(config.docstring_pattern.is_some());
        assert_eq!(config.conventions.len(), 2);
    }

    #[test]
    fn test_parse_minimal_config_no_optional_fields() {
        let yaml = r#"
language: bash
file_extensions:
  - sh
signature_patterns:
  function: "(?m)^(\\w+\\(\\))"
"#;
        let dir = tempdir().unwrap();
        let path = write_yaml(dir.path(), "bash.yaml", yaml);

        let config = load_lang_config(&path).unwrap();
        assert_eq!(config.language, "bash");
        assert!(config.docstring_pattern.is_none());
        assert!(config.conventions.is_empty());
    }

    #[test]
    fn test_load_lang_config_missing_file_returns_error() {
        let result = load_lang_config(Path::new("/nonexistent/path/config.yaml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_lang_config_invalid_yaml_returns_error() {
        let dir = tempdir().unwrap();
        let path = write_yaml(dir.path(), "bad.yaml", "{ invalid yaml: [missing bracket");

        let result = load_lang_config(&path);
        assert!(result.is_err());
    }

    // -------------------------------------------------------------------------
    // load_all_lang_configs
    // -------------------------------------------------------------------------

    #[test]
    fn test_load_all_from_directory_with_multiple_files() {
        let dir = tempdir().unwrap();
        write_yaml(dir.path(), "python.yaml", python_yaml());

        let rust_yaml = r#"
language: rust
file_extensions:
  - rs
signature_patterns:
  function: "(?m)^(pub fn \\w+)"
"#;
        write_yaml(dir.path(), "rust.yaml", rust_yaml);

        let configs = load_all_lang_configs(dir.path()).unwrap();
        assert_eq!(configs.len(), 2);

        let languages: Vec<&str> = configs.iter().map(|c| c.language.as_str()).collect();
        assert!(languages.contains(&"python"));
        assert!(languages.contains(&"rust"));
    }

    #[test]
    fn test_load_all_empty_directory_returns_empty_vec() {
        let dir = tempdir().unwrap();
        let configs = load_all_lang_configs(dir.path()).unwrap();
        assert!(configs.is_empty());
    }

    #[test]
    fn test_load_all_nonexistent_directory_returns_empty_vec() {
        let configs = load_all_lang_configs(Path::new("/nonexistent/lang/configs")).unwrap();
        assert!(configs.is_empty());
    }

    #[test]
    fn test_load_all_skips_non_yaml_files() {
        let dir = tempdir().unwrap();
        // Only .yaml/.yml should be loaded
        write_yaml(dir.path(), "python.yaml", python_yaml());
        fs::write(dir.path().join("readme.txt"), "not a config").unwrap();
        fs::write(dir.path().join("python.json"), r#"{"language":"json"}"#).unwrap();

        let configs = load_all_lang_configs(dir.path()).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].language, "python");
    }

    #[test]
    fn test_load_all_skips_invalid_yaml_files_gracefully() {
        let dir = tempdir().unwrap();
        write_yaml(dir.path(), "python.yaml", python_yaml());
        write_yaml(dir.path(), "broken.yaml", "{ broken: [");

        let configs = load_all_lang_configs(dir.path()).unwrap();
        // Only the valid one should be loaded
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].language, "python");
    }

    // -------------------------------------------------------------------------
    // find_for_extension (LangConfig)
    // -------------------------------------------------------------------------

    #[test]
    fn test_find_for_extension_matches() {
        let dir = tempdir().unwrap();
        write_yaml(dir.path(), "python.yaml", python_yaml());
        let configs = load_all_lang_configs(dir.path()).unwrap();

        let found = LangConfig::find_for_extension(&configs, "py");
        assert!(found.is_some());
        assert_eq!(found.unwrap().language, "python");
    }

    #[test]
    fn test_find_for_extension_no_match_returns_none() {
        let dir = tempdir().unwrap();
        write_yaml(dir.path(), "python.yaml", python_yaml());
        let configs = load_all_lang_configs(dir.path()).unwrap();

        let found = LangConfig::find_for_extension(&configs, "js");
        assert!(found.is_none());
    }

    #[test]
    fn test_find_for_extension_empty_configs_returns_none() {
        let found = LangConfig::find_for_extension(&[], "py");
        assert!(found.is_none());
    }

    // -------------------------------------------------------------------------
    // compile / compile_all
    // -------------------------------------------------------------------------

    #[test]
    fn test_compile_preserves_metadata() {
        let dir = tempdir().unwrap();
        let config_path = write_yaml(dir.path(), "python.yaml", python_yaml());
        let config = load_lang_config(&config_path).unwrap();
        let compiled = config.compile();

        assert_eq!(compiled.language, "python");
        assert_eq!(compiled.file_extensions, vec!["py"]);
        assert_eq!(compiled.conventions.len(), 2);
    }

    #[test]
    fn test_compile_all_returns_one_per_config() {
        let dir = tempdir().unwrap();
        write_yaml(dir.path(), "python.yaml", python_yaml());
        let rust_yaml = r#"
language: rust
file_extensions:
  - rs
signature_patterns:
  function: "(?m)^(pub fn \\w+)"
"#;
        write_yaml(dir.path(), "rust.yaml", rust_yaml);

        let configs = load_all_lang_configs(dir.path()).unwrap();
        let compiled = compile_all(&configs);
        assert_eq!(compiled.len(), 2);
    }

    // -------------------------------------------------------------------------
    // CompiledLangConfig::find_for_extension
    // -------------------------------------------------------------------------

    #[test]
    fn test_compiled_find_for_extension_matches() {
        let dir = tempdir().unwrap();
        write_yaml(dir.path(), "python.yaml", python_yaml());
        let configs = load_all_lang_configs(dir.path()).unwrap();
        let compiled = compile_all(&configs);

        let found = CompiledLangConfig::find_for_extension(&compiled, "py");
        assert!(found.is_some());
        assert_eq!(found.unwrap().language, "python");
    }

    #[test]
    fn test_compiled_find_for_extension_no_match_returns_none() {
        let dir = tempdir().unwrap();
        write_yaml(dir.path(), "python.yaml", python_yaml());
        let configs = load_all_lang_configs(dir.path()).unwrap();
        let compiled = compile_all(&configs);

        let found = CompiledLangConfig::find_for_extension(&compiled, "js");
        assert!(found.is_none());
    }

    // -------------------------------------------------------------------------
    // extract_signatures
    // -------------------------------------------------------------------------

    #[test]
    fn test_extract_signatures_python_functions_and_classes() {
        let dir = tempdir().unwrap();
        let config_path = write_yaml(dir.path(), "python.yaml", python_yaml());
        let config = load_lang_config(&config_path).unwrap();
        let compiled = config.compile();

        let py_content = r#""""Module docstring."""

def calculate(x, y):
    return x + y

def greet(name):
    print(f"Hello, {name}")

class Animal:
    pass

class Dog(Animal):
    pass
"#;
        let py_file = write_python_file(dir.path(), "sample.py", py_content);

        let sigs = compiled.extract_signatures(&py_file);

        // Should find both functions and both classes
        assert!(sigs.iter().any(|s| s.contains("calculate")), "missing calculate: {:?}", sigs);
        assert!(sigs.iter().any(|s| s.contains("greet")), "missing greet: {:?}", sigs);
        assert!(sigs.iter().any(|s| s.contains("Animal")), "missing Animal: {:?}", sigs);
        assert!(sigs.iter().any(|s| s.contains("Dog")), "missing Dog: {:?}", sigs);
    }

    #[test]
    fn test_extract_signatures_empty_file_returns_empty() {
        let dir = tempdir().unwrap();
        let config_path = write_yaml(dir.path(), "python.yaml", python_yaml());
        let config = load_lang_config(&config_path).unwrap();
        let compiled = config.compile();

        let py_file = write_python_file(dir.path(), "empty.py", "");

        let sigs = compiled.extract_signatures(&py_file);
        assert!(sigs.is_empty());
    }

    #[test]
    fn test_extract_signatures_missing_file_returns_empty() {
        let dir = tempdir().unwrap();
        let config_path = write_yaml(dir.path(), "python.yaml", python_yaml());
        let config = load_lang_config(&config_path).unwrap();
        let compiled = config.compile();

        let sigs = compiled.extract_signatures(Path::new("/nonexistent/file.py"));
        assert!(sigs.is_empty());
    }

    // -------------------------------------------------------------------------
    // extract_first_docstring
    // -------------------------------------------------------------------------

    #[test]
    fn test_extract_first_docstring_module_level() {
        let dir = tempdir().unwrap();
        let config_path = write_yaml(dir.path(), "python.yaml", python_yaml());
        let config = load_lang_config(&config_path).unwrap();
        let compiled = config.compile();

        let py_content = r#""""This is the module docstring."""

def foo():
    pass
"#;
        let py_file = write_python_file(dir.path(), "module.py", py_content);

        let docstring = compiled.extract_first_docstring(&py_file);
        assert!(docstring.is_some());
        assert!(docstring.unwrap().contains("module docstring"));
    }

    #[test]
    fn test_extract_first_docstring_no_docstring_returns_none() {
        let dir = tempdir().unwrap();
        let config_path = write_yaml(dir.path(), "python.yaml", python_yaml());
        let config = load_lang_config(&config_path).unwrap();
        let compiled = config.compile();

        let py_file = write_python_file(dir.path(), "no_doc.py", "x = 1\ny = 2\n");

        let docstring = compiled.extract_first_docstring(&py_file);
        assert!(docstring.is_none());
    }

    #[test]
    fn test_extract_first_docstring_no_pattern_returns_none() {
        let yaml = r#"
language: bash
file_extensions:
  - sh
signature_patterns:
  function: "(?m)^(\\w+\\(\\))"
"#;
        let dir = tempdir().unwrap();
        let config_path = write_yaml(dir.path(), "bash.yaml", yaml);
        let config = load_lang_config(&config_path).unwrap();
        let compiled = config.compile();

        let sh_file = write_python_file(dir.path(), "script.sh", "#!/bin/bash\necho hi\n");

        let docstring = compiled.extract_first_docstring(&sh_file);
        assert!(docstring.is_none());
    }

    #[test]
    fn test_extract_first_docstring_missing_file_returns_none() {
        let dir = tempdir().unwrap();
        let config_path = write_yaml(dir.path(), "python.yaml", python_yaml());
        let config = load_lang_config(&config_path).unwrap();
        let compiled = config.compile();

        let docstring = compiled.extract_first_docstring(Path::new("/nonexistent/file.py"));
        assert!(docstring.is_none());
    }
}
