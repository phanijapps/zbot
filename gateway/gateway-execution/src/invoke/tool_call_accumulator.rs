/// Record of a single tool call during execution.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCallRecord {
    pub tool_id: String,
    pub tool_name: String,
    pub args: serde_json::Value,
    pub result: Option<String>,
    pub error: Option<String>,
}

/// Accumulator for tool calls during execution.
#[derive(Default)]
pub struct ToolCallAccumulator {
    calls: Vec<ToolCallRecord>,
}

impl ToolCallAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_call(&mut self, tool_id: String, tool_name: String, args: serde_json::Value) {
        self.calls.push(ToolCallRecord {
            tool_id,
            tool_name,
            args,
            result: None,
            error: None,
        });
    }

    pub fn complete_call(&mut self, tool_id: &str, result: String, error: Option<String>) {
        if let Some(call) = self.calls.iter_mut().find(|c| c.tool_id == tool_id) {
            call.result = Some(result);
            call.error = error;
        }
    }

    pub fn to_json(&self) -> Option<String> {
        if self.calls.is_empty() {
            None
        } else {
            serde_json::to_string(&self.calls).ok()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }

    pub fn len(&self) -> usize {
        self.calls.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_and_complete_call() {
        let mut acc = ToolCallAccumulator::new();
        assert!(acc.is_empty());
        acc.start_call("t1".into(), "shell".into(), serde_json::json!({"cmd": "ls"}));
        assert_eq!(acc.len(), 1);
        acc.complete_call("t1", "file.txt".into(), None);
        let json = acc.to_json().expect("should serialize");
        assert!(json.contains("file.txt"));
    }

    #[test]
    fn complete_call_with_error() {
        let mut acc = ToolCallAccumulator::new();
        acc.start_call("t1".into(), "shell".into(), serde_json::json!({}));
        acc.complete_call("t1", String::new(), Some("not found".into()));
        let record = &serde_json::from_str::<Vec<ToolCallRecord>>(&acc.to_json().unwrap()).unwrap()[0];
        assert_eq!(record.error.as_deref(), Some("not found"));
    }

    #[test]
    fn to_json_returns_none_when_empty() {
        assert!(ToolCallAccumulator::new().to_json().is_none());
    }
}
