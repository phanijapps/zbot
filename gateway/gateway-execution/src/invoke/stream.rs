//! # Stream Handling
//!
//! Stream event processing and logging for agent execution.

// ============================================================================
// STREAM CONTEXT
// ============================================================================

pub use super::stream_context::StreamContext;

// ============================================================================
// RESPONSE ACCUMULATOR
// ============================================================================

pub use super::response_accumulator::{ResponseAccumulator, TURN_COMPLETE_MARKER};

// ============================================================================
// EVENT PROCESSING
// ============================================================================

pub use super::stream_event_processor::{broadcast_event, process_stream_event};

// ============================================================================
// TOOL CALL ACCUMULATOR
// ============================================================================

pub use super::tool_call_accumulator::{ToolCallAccumulator, ToolCallRecord};

// ============================================================================
// WARD SCAFFOLDING HELPERS
// ============================================================================

pub use super::ward_scaffolding::{collect_ward_setup_for_skill, collect_ward_setups_for_skills};

