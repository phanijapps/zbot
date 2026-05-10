/// Marker prefix for TurnComplete fallback content.
pub const TURN_COMPLETE_MARKER: &str = "\x00TURN_COMPLETE\x00";

/// Accumulator for building the final response from stream events.
#[derive(Default)]
pub struct ResponseAccumulator {
    content: String,
    turn_complete_fallback: Option<String>,
}

impl ResponseAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&mut self, content: &str) {
        if let Some(message) = content.strip_prefix(TURN_COMPLETE_MARKER) {
            self.turn_complete_fallback = Some(message.to_string());
            return;
        }
        self.content.push_str(content);
    }

    pub fn into_response(self) -> String {
        let trimmed = self.content.trim();
        if !trimmed.is_empty() {
            trimmed.to_string()
        } else if let Some(fallback) = self.turn_complete_fallback {
            fallback.trim().to_string()
        } else {
            String::new()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty() && self.turn_complete_fallback.is_none()
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_accumulator() {
        let mut acc = ResponseAccumulator::new();
        assert!(acc.is_empty());
        acc.append("Hello");
        assert!(!acc.is_empty());
        assert_eq!(acc.content(), "Hello");
        acc.append(" World");
        assert_eq!(acc.content(), "Hello World");
        assert_eq!(acc.into_response(), "Hello World");
    }

    #[test]
    fn test_response_accumulator_with_respond_tool() {
        let mut acc = ResponseAccumulator::new();
        acc.append("Initial response");
        acc.append("\n\nFrom respond tool");
        assert_eq!(acc.into_response(), "Initial response\n\nFrom respond tool");
    }

    #[test]
    fn turn_complete_used_as_fallback_when_no_tokens() {
        let mut acc = ResponseAccumulator::new();
        acc.append(&format!("{}final message", TURN_COMPLETE_MARKER));
        assert_eq!(acc.into_response(), "final message");
    }

    #[test]
    fn token_content_wins_over_turn_complete_fallback() {
        let mut acc = ResponseAccumulator::new();
        acc.append("token content");
        acc.append(&format!("{}turn complete", TURN_COMPLETE_MARKER));
        assert_eq!(acc.into_response(), "token content");
    }
}
