// ============================================================================
// EXECUTION TOOLS
// Python, Shell, LoadSkill, TODO, UpdatePlan, and ApplyPatch tools
// ============================================================================

pub mod apply_patch;
pub mod graph;
pub mod skills;
pub mod shell;
pub mod todos;
pub mod update_plan;

pub use graph::ExecutionGraphTool;
pub use shell::ShellTool;
pub use todos::TodoTool;
pub use update_plan::UpdatePlanTool;

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use zero_core::{Tool, ToolContext, Result};
use zero_core::FileSystemContext;

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

    async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let code = args.get("code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'code' parameter".to_string()))?;

        let python = self.fs.python_executable()
            .ok_or_else(|| zero_core::ZeroError::Tool("Python executable not configured".to_string()))?;

        tracing::debug!("Executing Python code ({} bytes)", code.len());

        // Create temp file for code
        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join(format!("agent_{}.py", uuid::Uuid::new_v4()));

        std::fs::write(&script_path, code)
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to write script: {}", e)))?;

        // Execute Python
        let output = tokio::process::Command::new(&python)
            .arg(&script_path)
            .output()
            .await
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to execute Python: {}", e)))?;

        // Clean up temp file
        let _ = std::fs::remove_file(&script_path);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(zero_core::ZeroError::Tool(format!("Python error: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        Ok(json!({
            "output": stdout,
        }))
    }
}
