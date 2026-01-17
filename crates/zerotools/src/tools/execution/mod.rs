// ============================================================================
// EXECUTION TOOLS
// Python and LoadSkill tools
// ============================================================================

pub mod skills;

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use agent_runtime::tools::{Tool, ToolExecError};
use agent_runtime::tools::context::ToolContext as BaseToolContext;
use agent_runtime::tools::error::ToolResult;
use agent_runtime::tools::builtin::FileSystemContext;

// ============================================================================
// PYTHON TOOL
// ============================================================================

/// Tool for executing Python code
pub struct PythonTool {
    /// File system context
    fs: Arc<dyn FileSystemContext>,
}

impl PythonTool {
    /// Create a new Python tool with file system context
    #[must_use]
    pub fn new(fs: Arc<dyn FileSystemContext>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl Tool for PythonTool {
    fn name(&self) -> &str {
        "python"
    }

    fn description(&self) -> &str {
        "Execute Python code in a virtual environment."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "Python code to execute"
                }
            },
            "required": ["code"]
        }))
    }

    async fn execute(&self, _ctx: Arc<BaseToolContext>, args: Value) -> ToolResult<Value> {
        let code = args.get("code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'code' parameter".to_string()))?;

        let python = self.fs.python_executable()
            .ok_or_else(|| ToolExecError::ExecutionFailed("Python executable not configured".to_string()))?;

        tracing::debug!("Executing Python code ({} bytes)", code.len());

        // Create temp file for code
        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join(format!("agent_{}.py", uuid::Uuid::new_v4()));

        std::fs::write(&script_path, code)
            .map_err(|e| ToolExecError::ExecutionFailed(format!("Failed to write script: {}", e)))?;

        // Execute Python
        let output = tokio::process::Command::new(&python)
            .arg(&script_path)
            .output()
            .await
            .map_err(|e| ToolExecError::ExecutionFailed(format!("Failed to execute Python: {}", e)))?;

        // Clean up temp file
        let _ = std::fs::remove_file(&script_path);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolExecError::ExecutionFailed(format!("Python error: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        Ok(json!({
            "output": stdout,
        }))
    }
}
