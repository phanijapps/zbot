//! Env-gated tool-result replay for the e2e harness.
//!
//! Activated by `ZBOT_REPLAY_DIR=/abs/path/to/fixture/tools`. When set,
//! every tool dispatch looks up a recorded result keyed by
//! `(execution_id, tool_index, tool_name, args_hash)` and returns it
//! instead of running the real tool.
//!
//! `ZBOT_REPLAY_STRICT=1` (default) panics on a miss. `ZBOT_REPLAY_STRICT=0`
//! falls through to real execution — useful while authoring new fixtures.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct ToolResultRecord {
    pub execution_id: String,
    pub tool_index: usize,
    pub tool_name: String,
    pub args_hash: String,
    pub result: String,
}

#[derive(Default)]
pub struct ReplayStore {
    by_exec: HashMap<String, Vec<ToolResultRecord>>,
    cursor: HashMap<String, usize>,
    strict: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LookupOutcome {
    Hit(String),
    MissStrict {
        exec_id: String,
        tool_index: usize,
    },
    MissLenient,
    Drift {
        expected_tool: String,
        got_tool: String,
    },
}

impl ReplayStore {
    pub fn from_path<P: AsRef<Path>>(path: P, strict: bool) -> std::io::Result<Self> {
        let contents = std::fs::read_to_string(path.as_ref())?;
        let mut by_exec: HashMap<String, Vec<ToolResultRecord>> = HashMap::new();
        for line in contents.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let rec: ToolResultRecord = serde_json::from_str(line)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            by_exec
                .entry(rec.execution_id.clone())
                .or_default()
                .push(rec);
        }
        Ok(ReplayStore {
            by_exec,
            cursor: HashMap::new(),
            strict,
        })
    }

    pub fn lookup(&mut self, exec_id: &str, tool_name: &str) -> LookupOutcome {
        let Some(list) = self.by_exec.get(exec_id) else {
            return if self.strict {
                LookupOutcome::MissStrict {
                    exec_id: exec_id.to_string(),
                    tool_index: 0,
                }
            } else {
                LookupOutcome::MissLenient
            };
        };
        let idx = *self.cursor.get(exec_id).unwrap_or(&0);
        if idx >= list.len() {
            return if self.strict {
                LookupOutcome::MissStrict {
                    exec_id: exec_id.to_string(),
                    tool_index: idx,
                }
            } else {
                LookupOutcome::MissLenient
            };
        }
        let rec = &list[idx];
        if rec.tool_name != tool_name {
            return LookupOutcome::Drift {
                expected_tool: rec.tool_name.clone(),
                got_tool: tool_name.to_string(),
            };
        }
        let out = rec.result.clone();
        self.cursor.insert(exec_id.to_string(), idx + 1);
        LookupOutcome::Hit(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_jsonl(lines: &[&str]) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        for l in lines {
            writeln!(f, "{}", l).unwrap();
        }
        f
    }

    #[test]
    fn hit_returns_recorded_result() {
        let f = temp_jsonl(&[
            r#"{"execution_id":"e1","tool_index":0,"tool_name":"shell","args_hash":"h","result":"ok"}"#,
        ]);
        let mut store = ReplayStore::from_path(f.path(), true).unwrap();
        assert_eq!(
            store.lookup("e1", "shell"),
            LookupOutcome::Hit("ok".to_string())
        );
    }

    #[test]
    fn strict_miss_when_no_records() {
        let f = temp_jsonl(&[]);
        let mut store = ReplayStore::from_path(f.path(), true).unwrap();
        assert!(matches!(
            store.lookup("e1", "shell"),
            LookupOutcome::MissStrict { .. }
        ));
    }

    #[test]
    fn lenient_miss_returns_miss_lenient() {
        let f = temp_jsonl(&[]);
        let mut store = ReplayStore::from_path(f.path(), false).unwrap();
        assert_eq!(store.lookup("e1", "shell"), LookupOutcome::MissLenient);
    }

    #[test]
    fn drift_when_tool_name_differs() {
        let f = temp_jsonl(&[
            r#"{"execution_id":"e1","tool_index":0,"tool_name":"shell","args_hash":"h","result":"ok"}"#,
        ]);
        let mut store = ReplayStore::from_path(f.path(), true).unwrap();
        let outcome = store.lookup("e1", "read_file");
        assert!(matches!(
            outcome,
            LookupOutcome::Drift { ref expected_tool, ref got_tool }
                if expected_tool == "shell" && got_tool == "read_file"
        ));
    }
}
