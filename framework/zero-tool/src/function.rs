//! # Function Tool
//!
//! Ergonomic wrapper for creating tools from async functions.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use zero_core::{Result, Tool, ToolContext};

/// A tool created from an async function.
///
/// # Example
///
/// ```
/// use zero_tool::FunctionTool;
/// use std::sync::Arc;
///
/// let tool = FunctionTool::new(
///     "get_weather",
///     "Get the current weather for a location",
///     |_ctx, args| {
///         Box::pin(async move {
///             let location = args["location"].as_str().unwrap_or("unknown");
///             Ok(serde_json::json!({
///                 "location": location,
///                 "temperature": 72,
///                 "condition": "sunny"
///             }))
///         })
///     },
/// );
/// ```
pub struct FunctionTool {
    name: String,
    description: String,
    parameters: Option<Value>,
    handler: Arc<
        dyn Fn(Arc<dyn ToolContext>, Value) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
            + Send
            + Sync,
    >,
}

impl FunctionTool {
    /// Create a new function tool.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool
    /// * `description` - A description of what the tool does
    /// * `handler` - An async function that takes a context and arguments
    pub fn new<
        F: Fn(Arc<dyn ToolContext>, Value) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
            + Send
            + Sync
            + 'static,
    >(
        name: impl Into<String>,
        description: impl Into<String>,
        handler: F,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: None,
            handler: Arc::new(handler),
        }
    }

    /// Set the JSON Schema for the tool's parameters.
    pub fn with_parameters(mut self, parameters: Value) -> Self {
        self.parameters = Some(parameters);
        self
    }

    /// Set the JSON Schema for the tool's response.
    pub fn with_response_schema(self, _schema: Value) -> Self {
        // Response schema is stored but not currently used
        self
    }

    /// Create a function tool from a simpler signature.
    ///
    /// This is a convenience method for tools that don't need the full context.
    ///
    /// # Example
    ///
    /// ```
    /// use zero_tool::FunctionTool;
    ///
    /// let tool = FunctionTool::simple(
    ///     "add",
    ///     "Add two numbers",
    ///     |args| {
    ///         Box::pin(async move {
    ///             let a = args["a"].as_i64().unwrap_or(0);
    ///             let b = args["b"].as_i64().unwrap_or(0);
    ///             Ok(serde_json::json!(a + b))
    ///         })
    ///     },
    /// );
    /// ```
    pub fn simple<
        F: Fn(Value) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>> + Send + Sync + 'static,
    >(
        name: impl Into<String>,
        description: impl Into<String>,
        handler: F,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: None,
            handler: Arc::new(move |_ctx, args| handler(args)),
        }
    }
}

#[async_trait]
impl Tool for FunctionTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> Option<Value> {
        self.parameters.clone()
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        (self.handler)(ctx, args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock ToolContext for testing
    struct MockContext;

    impl zero_core::context::ReadonlyContext for MockContext {
        fn invocation_id(&self) -> &str {
            "test"
        }

        fn agent_name(&self) -> &str {
            "test_agent"
        }

        fn user_id(&self) -> &str {
            "test_user"
        }

        fn app_name(&self) -> &str {
            "test_app"
        }

        fn session_id(&self) -> &str {
            "test_session"
        }

        fn branch(&self) -> &str {
            "main"
        }

        fn user_content(&self) -> &zero_core::types::Content {
            static CONTENT: zero_core::types::Content = zero_core::types::Content {
                role: String::new(),
                parts: Vec::new(),
            };
            &CONTENT
        }
    }

    impl zero_core::context::CallbackContext for MockContext {
        fn get_state(&self, _key: &str) -> Option<Value> {
            None
        }

        fn set_state(&self, _key: String, _value: Value) {
            // No-op for testing
        }
    }

    impl zero_core::context::ToolContext for MockContext {
        fn function_call_id(&self) -> String {
            "call_123".to_string()
        }

        fn actions(&self) -> zero_core::EventActions {
            zero_core::EventActions::default()
        }

        fn set_actions(&self, _actions: zero_core::EventActions) {
            // No-op for testing
        }
    }

    #[tokio::test]
    async fn test_function_tool_basic() {
        let tool = FunctionTool::new(
            "echo",
            "Echo back the input",
            |_ctx, args| {
                Box::pin(async move { Ok(args) })
            },
        );

        assert_eq!(tool.name(), "echo");
        assert_eq!(tool.description(), "Echo back the input");

        let ctx = Arc::new(MockContext);
        let input = serde_json::json!({"hello": "world"});
        let result = tool.execute(ctx, input.clone()).await.unwrap();

        assert_eq!(result, input);
    }

    #[tokio::test]
    async fn test_function_tool_with_parameters() {
        let tool = FunctionTool::new(
            "calculate",
            "Perform a calculation",
            |_ctx, args| {
                Box::pin(async move {
                    let a = args["a"].as_i64().unwrap_or(0);
                    let b = args["b"].as_i64().unwrap_or(0);
                    Ok(serde_json::json!(a + b))
                })
            },
        )
        .with_parameters(serde_json::json!({
            "type": "object",
            "properties": {
                "a": {"type": "integer"},
                "b": {"type": "integer"}
            },
            "required": ["a", "b"]
        }));

        let ctx = Arc::new(MockContext);
        let result = tool
            .execute(ctx, serde_json::json!({"a": 5, "b": 3}))
            .await
            .unwrap();

        assert_eq!(result, serde_json::json!(8));
    }

    #[tokio::test]
    async fn test_function_tool_simple() {
        let tool = FunctionTool::simple("uppercase", "Convert to uppercase", |args| {
            Box::pin(async move {
                let text = args["text"].as_str().unwrap_or("");
                Ok(serde_json::json!(text.to_uppercase()))
            })
        });

        let ctx = Arc::new(MockContext);
        let result = tool
            .execute(ctx, serde_json::json!({"text": "hello"}))
            .await
            .unwrap();

        assert_eq!(result, serde_json::json!("HELLO"));
    }
}
