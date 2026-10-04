use super::{project_arguments, HookEvent, HookFailure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_EVENT_BYTES: usize = 64 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 32 * 1024;
pub const MAX_CONTEXT_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookMode {
    Chat,
    Research,
}
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookRunStatus {
    Completed,
    Failed,
    Blocked,
    Cancelled,
}
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookOperationStatus {
    Completed,
    Failed,
    Cancelled,
}
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InvalidToolCategory {
    UnknownTool,
    InvalidArguments,
    PolicyDenied,
}

/// Only event-specific fields can be encoded. Instruction/recall/provider configs have no slot.
#[derive(Clone, Serialize)]
#[serde(untagged)]
pub enum HookEventData {
    SessionStart {},
    UserPrompt {
        prompt: String,
    },
    RunStart {
        mode: HookMode,
    },
    RunEnd {
        status: HookRunStatus,
    },
    BeforeModel {
        provider: String,
        model: String,
    },
    AfterModel {
        provider: String,
        model: String,
        status: HookOperationStatus,
    },
    BeforeTool {
        tool: String,
        arguments: Value,
        arguments_redacted: bool,
    },
    AfterTool {
        tool: String,
        status: HookOperationStatus,
    },
    InvalidToolCall {
        tool: Option<String>,
        error: InvalidToolCategory,
    },
}
pub struct HookEventPayload {
    pub event_id: String,
    pub invocation_id: String,
    pub session_id: String,
    pub agent_id: String,
    pub run_id: Option<String>,
    pub turn_id: Option<String>,
    pub tool_call_id: Option<String>,
    pub occurred_at: String,
    pub data: HookEventData,
}
impl HookEventPayload {
    pub fn new(
        invocation_id: String,
        session_id: String,
        agent_id: String,
        data: HookEventData,
    ) -> Self {
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            invocation_id,
            session_id,
            agent_id,
            run_id: None,
            turn_id: None,
            tool_call_id: None,
            occurred_at: chrono::Utc::now().to_rfc3339(),
            data,
        }
    }
    pub fn event(&self) -> HookEvent {
        match &self.data {
            HookEventData::SessionStart {} => HookEvent::SessionStart,
            HookEventData::UserPrompt { .. } => HookEvent::UserPrompt,
            HookEventData::RunStart { .. } => HookEvent::RunStart,
            HookEventData::RunEnd { .. } => HookEvent::RunEnd,
            HookEventData::BeforeModel { .. } => HookEvent::BeforeModel,
            HookEventData::AfterModel { .. } => HookEvent::AfterModel,
            HookEventData::BeforeTool { .. } => HookEvent::BeforeTool,
            HookEventData::AfterTool { .. } => HookEvent::AfterTool,
            HookEventData::InvalidToolCall { .. } => HookEvent::InvalidToolCall,
        }
    }
    pub fn encode(&self, secrets: &[String]) -> Result<Vec<u8>, HookFailure> {
        for id in [
            &self.event_id,
            &self.invocation_id,
            &self.session_id,
            &self.agent_id,
        ] {
            if !bounded_string(id, 1, 128) {
                return Err(HookFailure::InvalidEvent);
            }
        }
        for id in [&self.run_id, &self.turn_id, &self.tool_call_id]
            .into_iter()
            .flatten()
        {
            if !bounded_string(id, 1, 128) {
                return Err(HookFailure::InvalidEvent);
            }
        }
        if chrono::DateTime::parse_from_rfc3339(&self.occurred_at).is_err() {
            return Err(HookFailure::InvalidEvent);
        }
        let data = match &self.data {
            HookEventData::BeforeTool {
                tool,
                arguments,
                arguments_redacted,
            } => {
                let projected = project_arguments(arguments, secrets);
                HookEventData::BeforeTool {
                    tool: tool.clone(),
                    arguments: projected.arguments,
                    arguments_redacted: *arguments_redacted || projected.redacted,
                }
            }
            data => data.clone(),
        };
        match &data {
            HookEventData::UserPrompt { prompt } if !bounded_string(prompt, 0, 16384) => {
                return Err(HookFailure::InvalidEvent)
            }
            HookEventData::BeforeModel { provider, model }
            | HookEventData::AfterModel {
                provider, model, ..
            } if !bounded_string(provider, 1, 128) || !bounded_string(model, 1, 256) => {
                return Err(HookFailure::InvalidEvent)
            }
            HookEventData::BeforeTool { tool, .. } if !bounded_string(tool, 1, 128) => {
                return Err(HookFailure::InvalidEvent)
            }
            HookEventData::AfterTool { tool, .. } if !bounded_string(tool, 1, 128) => {
                return Err(HookFailure::InvalidEvent)
            }
            HookEventData::InvalidToolCall {
                tool: Some(tool), ..
            } if !bounded_string(tool, 1, 128) => return Err(HookFailure::InvalidEvent),
            _ => {}
        }
        #[derive(Serialize)]
        struct WireEvent<'a> {
            version: u8,
            event_id: &'a str,
            invocation_id: &'a str,
            session_id: &'a str,
            agent_id: &'a str,
            run_id: &'a Option<String>,
            turn_id: &'a Option<String>,
            tool_call_id: &'a Option<String>,
            event: HookEvent,
            occurred_at: &'a str,
            data: &'a HookEventData,
        }
        let wire = WireEvent {
            version: 1,
            event_id: &self.event_id,
            invocation_id: &self.invocation_id,
            session_id: &self.session_id,
            agent_id: &self.agent_id,
            run_id: &self.run_id,
            turn_id: &self.turn_id,
            tool_call_id: &self.tool_call_id,
            event: self.event(),
            occurred_at: &self.occurred_at,
            data: &data,
        };
        let bytes = serde_json::to_vec(&wire).map_err(|_| HookFailure::InvalidEvent)?;
        if bytes.len() > MAX_EVENT_BYTES {
            return Err(HookFailure::InputTooLarge);
        }
        Ok(bytes)
    }
}
fn bounded_string(string: &str, minimum: usize, maximum: usize) -> bool {
    (minimum..=maximum).contains(&string.chars().take(maximum + 1).count())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub enum HookAction {
    Continue,
    Block,
}
impl TryFrom<String> for HookAction {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "continue" => Ok(Self::Continue),
            "block" => Ok(Self::Block),
            _ => Err("unsupported action"),
        }
    }
}

/// Private reason/context never appear in Debug or sanitized outcome metadata.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookResponse {
    #[serde(deserialize_with = "super::config::schema_integer")]
    version: u64,
    pub action: HookAction,
    #[serde(default, deserialize_with = "optional_string")]
    context: Option<String>,
    #[serde(default, deserialize_with = "optional_string")]
    reason: Option<String>,
}
fn optional_string<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}
impl HookResponse {
    pub fn parse(bytes: &[u8], event: HookEvent) -> Result<Self, HookFailure> {
        if bytes.len() > MAX_OUTPUT_BYTES {
            return Err(HookFailure::OutputTooLarge);
        }
        let response = if bytes.is_empty() {
            Self {
                version: 1,
                action: HookAction::Continue,
                context: None,
                reason: None,
            }
        } else {
            serde_json::from_slice(bytes).map_err(|_| HookFailure::InvalidResponse)?
        };
        if response.version != 1
            || response
                .reason
                .as_ref()
                .is_some_and(|reason| reason.chars().count() > 160)
            || (response.action == HookAction::Block
                && (!event.allows_block() || response.context.is_some()))
            || (response.context.is_some() && !event.allows_context())
        {
            return Err(HookFailure::InvalidResponse);
        }
        if response
            .context
            .as_ref()
            .is_some_and(|context| context.len() > MAX_CONTEXT_BYTES)
        {
            return Err(HookFailure::ContextTooLarge);
        }
        Ok(response)
    }
    pub fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }
}
impl std::fmt::Debug for HookResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HookResponse")
            .field("action", &self.action)
            .finish_non_exhaustive()
    }
}
#[derive(Default, Clone)]
pub struct HookContextBudget {
    used: usize,
}
impl HookContextBudget {
    pub(super) fn restored(used: usize) -> Self {
        Self {
            used: used.min(MAX_CONTEXT_BYTES),
        }
    }
    pub(super) fn used(&self) -> usize {
        self.used
    }

    /// Charge once after a successful response; rejection does not change the budget.
    pub fn accept(&mut self, response: &HookResponse) -> Result<Option<String>, HookFailure> {
        match response.context() {
            Some(context) => {
                let used = self
                    .used
                    .checked_add(context.len())
                    .filter(|used| *used <= MAX_CONTEXT_BYTES)
                    .ok_or(HookFailure::ContextTooLarge)?;
                self.used = used;
                Ok(Some(context.to_owned()))
            }
            None => Ok(None),
        }
    }
}
#[cfg(test)]
mod tests;
